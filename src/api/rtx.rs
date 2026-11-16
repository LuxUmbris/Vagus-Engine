use nucleus::rtx::{
    hardware_ray_tracing_supported, Camera as NucleusCamera, RtxRenderer as NucleusRenderer,
    Triangle as NucleusTriangle,
};

/// A perspective camera used to generate primary rays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    origin: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    vertical_fov_degrees: f32,
}

impl Camera {
    /// Creates a look-at camera; the vertical field of view is in degrees and must be in (0, 180).
    pub fn new(
        origin: [f32; 3],
        target: [f32; 3],
        up: [f32; 3],
        vertical_fov_degrees: f32,
    ) -> Result<Self, String> {
        if [origin, target, up]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
            || !vertical_fov_degrees.is_finite()
        {
            return Err("camera parameters must contain only finite values".into());
        }
        if origin == target || up == [0.0; 3] {
            return Err("camera direction and up vectors must be nonzero".into());
        }
        let direction = [
            f64::from(target[0]) - f64::from(origin[0]),
            f64::from(target[1]) - f64::from(origin[1]),
            f64::from(target[2]) - f64::from(origin[2]),
        ];
        let up_f64 = up.map(f64::from);
        let right = [
            direction[1] * up_f64[2] - direction[2] * up_f64[1],
            direction[2] * up_f64[0] - direction[0] * up_f64[2],
            direction[0] * up_f64[1] - direction[1] * up_f64[0],
        ];
        if right == [0.0; 3] {
            return Err("camera up vector must not be parallel to its viewing direction".into());
        }
        if !(0.0 < vertical_fov_degrees && vertical_fov_degrees < 180.0) {
            return Err("vertical field of view must be between 0 and 180 degrees".into());
        }
        Ok(Self {
            origin,
            target,
            up,
            vertical_fov_degrees,
        })
    }

    fn backend_data(self) -> NucleusCamera {
        NucleusCamera {
            origin: self.origin,
            target: self.target,
            up: self.up,
            vertical_fov_degrees: self.vertical_fov_degrees,
        }
    }
}

/// A single non-indexed triangle in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    vertices: [[f32; 3]; 3],
}

impl Triangle {
    pub fn new(vertices: [[f32; 3]; 3]) -> Result<Self, String> {
        if vertices
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err("triangle vertices must contain only finite values".into());
        }
        Ok(Self { vertices })
    }

    pub fn vertices(&self) -> &[[f32; 3]; 3] {
        &self.vertices
    }
}

/// A collection of triangles uploaded to the hardware ray-tracing backend.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    triangles: Vec<Triangle>,
}

impl Scene {
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.is_empty() {
            return Err("ray-tracing scenes must contain at least one triangle".into());
        }
        Ok(Self { triangles })
    }

    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }
}

/// RGBA8 image returned by a completed hardware ray dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    width: u32,
    height: u32,
    rgba: Vec<[u8; 4]>,
}

impl Frame {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.rgba
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.rgba
            .get(y as usize * self.width as usize + x as usize)
            .copied()
    }
}

/// Offscreen Vulkan ray tracer using the vendor-neutral KHR ray-tracing path.
pub struct RtxRenderer {
    backend: NucleusRenderer,
    width: u32,
    height: u32,
}

impl RtxRenderer {
    /// Returns whether a Vulkan device exposes the required KHR hardware features.
    pub fn is_hardware_supported() -> bool {
        hardware_ray_tracing_supported()
    }

    /// Creates an offscreen target, failing if KHR hardware ray tracing is unavailable.
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("ray-tracing target dimensions must be positive".into());
        }
        let pixel_count = usize::try_from(width)
            .ok()
            .and_then(|width| usize::try_from(height).ok()?.checked_mul(width))
            .ok_or_else(|| "ray-tracing target dimensions overflow address space".to_owned())?;
        pixel_count
            .checked_mul(4)
            .ok_or_else(|| "ray-tracing target byte size overflows address space".to_owned())?;
        let backend = NucleusRenderer::new(width, height)?;
        Ok(Self {
            backend,
            width,
            height,
        })
    }

    /// Traces the scene and returns top-to-bottom, left-to-right RGBA8 pixels.
    pub fn render(&mut self, scene: &Scene, camera: &Camera) -> Result<Frame, String> {
        let mut triangles = Vec::new();
        triangles
            .try_reserve_exact(scene.triangles.len())
            .map_err(|error| format!("could not prepare ray-tracing geometry: {error}"))?;
        triangles.extend(scene.triangles.iter().map(|triangle| NucleusTriangle {
            vertices: triangle.vertices,
        }));
        let rgba = self.backend.trace(&triangles, camera.backend_data())?;
        let expected_pixels = self.width as usize * self.height as usize;
        if rgba.len() != expected_pixels {
            return Err(format!(
                "ray-tracing backend returned {} pixels for a {}x{} target",
                rgba.len(),
                self.width,
                self.height
            ));
        }
        Ok(Frame {
            width: self.width,
            height: self.height,
            rgba,
        })
    }
}
