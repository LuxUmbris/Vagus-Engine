//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use nucleus::rtx::{
    Camera as NucleusCamera, RtxRenderer as NucleusRenderer, Triangle as NucleusTriangle,
    hardware_ray_tracing_supported,
};

/// A perspective camera used to generate primary rays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    origin: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    vertical_fov_degrees: f32,
    near_plane: f32,
    far_plane: f32,
}

impl Camera {
    /// Creates a look-at camera with a vertical field of view and positive near/far distances.
    pub fn new(
        origin: [f32; 3],
        target: [f32; 3],
        up: [f32; 3],
        vertical_fov_degrees: f32,
        near_plane: f32,
        far_plane: f32,
    ) -> Result<Self, String> {
        if [origin, target, up]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
            || !vertical_fov_degrees.is_finite()
            || !near_plane.is_finite()
            || !far_plane.is_finite()
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
        if near_plane <= 0.0 || far_plane <= near_plane {
            return Err("camera far plane must be greater than a positive near plane".into());
        }
        Ok(Self {
            origin,
            target,
            up,
            vertical_fov_degrees,
            near_plane,
            far_plane,
        })
    }

    fn backend_data(self) -> NucleusCamera {
        NucleusCamera {
            origin: self.origin,
            target: self.target,
            up: self.up,
            vertical_fov_degrees: self.vertical_fov_degrees,
            near_plane: self.near_plane,
            far_plane: self.far_plane,
        }
    }

    pub(crate) fn projection(&self, width: u32, height: u32) -> Result<CameraProjection, String> {
        if width == 0 || height == 0 {
            return Err("camera projection viewport dimensions must be positive".into());
        }

        let origin = self.origin.map(f64::from);
        let target = self.target.map(f64::from);
        let direction = [
            target[0] - origin[0],
            target[1] - origin[1],
            target[2] - origin[2],
        ];
        let forward = normalize3(direction)?;
        let right = normalize3(cross3(self.up.map(f64::from), forward))?;
        let up = cross3(forward, right);
        Ok(CameraProjection {
            origin,
            forward,
            right,
            up,
            tangent: (f64::from(self.vertical_fov_degrees).to_radians() * 0.5).tan(),
            aspect: f64::from(width) / f64::from(height),
            #[cfg(test)]
            width: f64::from(width),
            #[cfg(test)]
            height: f64::from(height),
            near_plane: f64::from(self.near_plane),
            far_plane: f64::from(self.far_plane),
        })
    }
}

pub(crate) struct CameraProjection {
    origin: [f64; 3],
    forward: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    tangent: f64,
    aspect: f64,
    #[cfg(test)]
    width: f64,
    #[cfg(test)]
    height: f64,
    near_plane: f64,
    far_plane: f64,
}

impl CameraProjection {
    #[cfg(test)]
    pub(crate) fn project_triangle_clipped(
        &self,
        vertices: &[[f32; 3]; 3],
    ) -> Result<([[[f32; 3]; 3]; 7], usize), String> {
        let (polygon, polygon_len) = self.clipped_polygon(vertices)?;
        let mut triangles = [[[0.0; 3]; 3]; 7];
        let triangle_count = polygon_len.saturating_sub(2);
        for index in 1..polygon_len.saturating_sub(1) {
            triangles[index - 1] = [
                self.project_view_vertex(polygon[0])?,
                self.project_view_vertex(polygon[index])?,
                self.project_view_vertex(polygon[index + 1])?,
            ];
        }
        Ok((triangles, triangle_count))
    }

    pub(crate) fn camera_space_triangle_clipped(
        &self,
        vertices: &[[f32; 3]; 3],
    ) -> Result<([[[f32; 3]; 3]; 7], usize), String> {
        let (polygon, polygon_len) = self.clipped_polygon(vertices)?;
        let mut triangles = [[[0.0; 3]; 3]; 7];
        let triangle_count = polygon_len.saturating_sub(2);
        for index in 1..polygon_len.saturating_sub(1) {
            for (destination, relative) in triangles[index - 1].iter_mut().zip([
                polygon[0],
                polygon[index],
                polygon[index + 1],
            ]) {
                *destination = [
                    dot3(relative, self.right) as f32,
                    dot3(relative, self.up) as f32,
                    dot3(relative, self.forward) as f32,
                ];
                if destination.iter().any(|coordinate| !coordinate.is_finite()) {
                    return Err("camera-space vertex exceeds GPU range".into());
                }
            }
        }
        Ok((triangles, triangle_count))
    }

    pub(crate) fn gpu_projection_parameters(&self) -> Result<[f32; 4], String> {
        let parameters = [
            self.tangent * self.aspect,
            self.tangent,
            self.near_plane,
            self.far_plane,
        ]
        .map(|value| value as f32);
        if parameters.iter().any(|value| !value.is_finite()) {
            return Err("camera projection parameters exceed GPU range".into());
        }
        Ok(parameters)
    }

    pub(crate) fn gpu_camera_projection(&self) -> Result<nucleus::GpuCameraProjection, String> {
        let to_f32 = |values: [f64; 3]| {
            let values = values.map(|value| value as f32);
            if values.iter().all(|value| value.is_finite()) {
                Ok(values)
            } else {
                Err("camera view transform exceeds GPU range".to_owned())
            }
        };
        nucleus::GpuCameraProjection::new(
            to_f32(self.origin)?,
            to_f32(self.right)?,
            to_f32(self.up)?,
            to_f32(self.forward)?,
            self.gpu_projection_parameters()?,
        )
    }

    pub(crate) fn camera_space_triangle(
        &self,
        vertices: &[[f32; 3]; 3],
    ) -> Result<[[f32; 3]; 3], String> {
        let camera_space = vertices.map(|vertex| {
            let relative = [
                f64::from(vertex[0]) - self.origin[0],
                f64::from(vertex[1]) - self.origin[1],
                f64::from(vertex[2]) - self.origin[2],
            ];
            [
                dot3(relative, self.right) as f32,
                dot3(relative, self.up) as f32,
                dot3(relative, self.forward) as f32,
            ]
        });
        if camera_space
            .into_iter()
            .flatten()
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err("camera-space vertex exceeds GPU range".into());
        }
        Ok(camera_space)
    }

    pub(crate) fn triangle_outside_frustum(&self, points: &[[f32; 3]; 3]) -> bool {
        let points = points.map(|point| point.map(f64::from));
        let horizontal_extent = self.tangent * self.aspect;
        let vertical_extent = self.tangent;
        let outside_planes = [
            points.iter().all(|point| point[2] < self.near_plane),
            points.iter().all(|point| point[2] > self.far_plane),
            points
                .iter()
                .all(|point| point[0] > point[2] * horizontal_extent),
            points
                .iter()
                .all(|point| point[0] < -point[2] * horizontal_extent),
            points
                .iter()
                .all(|point| point[1] > point[2] * vertical_extent),
            points
                .iter()
                .all(|point| point[1] < -point[2] * vertical_extent),
        ];
        outside_planes.into_iter().any(|outside| outside)
    }

    pub(crate) fn bounds_outside_frustum(&self, bounds: ([f32; 3], [f32; 3])) -> bool {
        let (minimum, maximum) = bounds;
        let corners: [[f64; 3]; 8] = std::array::from_fn(|index| {
            std::array::from_fn(|axis| {
                f64::from(if index & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }) - self.origin[axis]
            })
        });
        let horizontal_extent = self.tangent * self.aspect;
        let planes = [
            (self.forward, -self.near_plane),
            (self.forward.map(|component| -component), self.far_plane),
            (
                std::array::from_fn(|axis| {
                    self.forward[axis] * horizontal_extent + self.right[axis]
                }),
                0.0,
            ),
            (
                std::array::from_fn(|axis| {
                    self.forward[axis] * horizontal_extent - self.right[axis]
                }),
                0.0,
            ),
            (
                std::array::from_fn(|axis| self.forward[axis] * self.tangent + self.up[axis]),
                0.0,
            ),
            (
                std::array::from_fn(|axis| self.forward[axis] * self.tangent - self.up[axis]),
                0.0,
            ),
        ];

        planes.into_iter().any(|(normal, offset)| {
            corners.iter().all(|corner| {
                let distance = dot3(*corner, normal) + offset;
                let scale = corner
                    .iter()
                    .zip(normal)
                    .map(|(coordinate, component)| (coordinate * component).abs())
                    .sum::<f64>()
                    + offset.abs();
                let tolerance = scale.max(1.0) * f64::from(f32::EPSILON) * 8.0;
                distance < -tolerance
            })
        })
    }

    pub(crate) fn normalized_depth(&self, camera_depth: f32) -> Result<f32, String> {
        let depth = f64::from(camera_depth).clamp(self.near_plane, self.far_plane);
        let projected = self.far_plane * (depth - self.near_plane)
            / (depth * (self.far_plane - self.near_plane));
        let projected = projected as f32;
        if !projected.is_finite() || !(0.0..=1.0).contains(&projected) {
            return Err("camera depth exceeds GPU range".into());
        }
        Ok(projected)
    }

    fn clipped_polygon(&self, vertices: &[[f32; 3]; 3]) -> Result<([[f64; 3]; 9], usize), String> {
        let relative = vertices.map(|vertex| {
            [
                f64::from(vertex[0]) - self.origin[0],
                f64::from(vertex[1]) - self.origin[1],
                f64::from(vertex[2]) - self.origin[2],
            ]
        });
        let horizontal_extent = self.tangent * self.aspect;
        let clipping_planes: [([f64; 3], f64); 6] = [
            (self.forward, -self.near_plane),
            (self.forward.map(|component| -component), self.far_plane),
            (
                std::array::from_fn(|axis| {
                    self.forward[axis] * horizontal_extent + self.right[axis]
                }),
                0.0,
            ),
            (
                std::array::from_fn(|axis| {
                    self.forward[axis] * horizontal_extent - self.right[axis]
                }),
                0.0,
            ),
            (
                std::array::from_fn(|axis| self.forward[axis] * self.tangent + self.up[axis]),
                0.0,
            ),
            (
                std::array::from_fn(|axis| self.forward[axis] * self.tangent - self.up[axis]),
                0.0,
            ),
        ];
        let mut polygon = [[0.0; 3]; 9];
        let mut polygon_len = 0;

        for vertex in relative {
            push_clip_vertex(&mut polygon, &mut polygon_len, vertex);
        }
        for (normal, offset) in clipping_planes {
            let previous_polygon = polygon;
            let previous_len = polygon_len;
            polygon = [[0.0; 3]; 9];
            polygon_len = 0;
            if previous_len == 0 {
                break;
            }
            for index in 0..previous_len {
                let previous = previous_polygon[(index + previous_len - 1) % previous_len];
                let current = previous_polygon[index];
                let previous_distance = dot3(previous, normal) + offset;
                let current_distance = dot3(current, normal) + offset;
                let previous_inside = previous_distance >= 0.0;
                let current_inside = current_distance >= 0.0;

                match (previous_inside, current_inside) {
                    (true, true) => push_clip_vertex(&mut polygon, &mut polygon_len, current),
                    (true, false) => push_clip_vertex(
                        &mut polygon,
                        &mut polygon_len,
                        clip_intersection(previous, current, previous_distance, current_distance),
                    ),
                    (false, true) => {
                        let intersection = clip_intersection(
                            previous,
                            current,
                            previous_distance,
                            current_distance,
                        );
                        push_clip_vertex(&mut polygon, &mut polygon_len, intersection);
                        if current_distance > 0.0 {
                            push_clip_vertex(&mut polygon, &mut polygon_len, current);
                        }
                    }
                    (false, false) => {}
                }
            }
        }

        Ok((polygon, polygon_len))
    }

    #[cfg(test)]
    fn project_view_vertex(&self, relative: [f64; 3]) -> Result<[f32; 3], String> {
        let measured_depth = dot3(relative, self.forward);
        let tolerance = self.near_plane * 1e-10;
        if !measured_depth.is_finite() || measured_depth + tolerance < self.near_plane {
            return Err("camera-frustum clipping produced a vertex behind the near plane".into());
        }
        let view_depth = measured_depth.clamp(self.near_plane, self.far_plane);
        let ndc_x = dot3(relative, self.right) / (view_depth * self.tangent * self.aspect);
        let ndc_y = dot3(relative, self.up) / (view_depth * self.tangent);
        let projected = [
            (ndc_x + 1.0) * self.width * 0.5,
            (1.0 - ndc_y) * self.height * 0.5,
            self.far_plane * (view_depth - self.near_plane)
                / (view_depth * (self.far_plane - self.near_plane)),
        ]
        .map(|value| value as f32);
        if projected.iter().any(|value| !value.is_finite()) {
            return Err("camera projection exceeds GPU vertex range".into());
        }
        Ok(projected)
    }
}

fn push_clip_vertex(polygon: &mut [[f64; 3]; 9], length: &mut usize, vertex: [f64; 3]) {
    debug_assert!(*length < polygon.len());
    polygon[*length] = vertex;
    *length += 1;
}

fn clip_intersection(
    previous: [f64; 3],
    current: [f64; 3],
    previous_distance: f64,
    current_distance: f64,
) -> [f64; 3] {
    let fraction = (previous_distance / (previous_distance - current_distance)).clamp(0.0, 1.0);
    std::array::from_fn(|axis| previous[axis] + (current[axis] - previous[axis]) * fraction)
}

fn dot3(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    lhs[0] * rhs[0] + lhs[1] * rhs[1] + lhs[2] * rhs[2]
}

fn cross3(lhs: [f64; 3], rhs: [f64; 3]) -> [f64; 3] {
    [
        lhs[1] * rhs[2] - lhs[2] * rhs[1],
        lhs[2] * rhs[0] - lhs[0] * rhs[2],
        lhs[0] * rhs[1] - lhs[1] * rhs[0],
    ]
}

fn normalize3(vector: [f64; 3]) -> Result<[f64; 3], String> {
    let length = dot3(vector, vector).sqrt();
    if !length.is_finite() || length == 0.0 {
        return Err("camera basis vectors must be finite and nonzero".into());
    }
    Ok(vector.map(|component| component / length))
}

/// A single non-indexed triangle in world space.
#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    vertices: [[f32; 3]; 3],
    uvs: [[f32; 2]; 3],
    flat_normal: Option<[f32; 3]>,
}

impl PartialEq for Triangle {
    fn eq(&self, other: &Self) -> bool {
        self.vertices == other.vertices && self.uvs == other.uvs
    }
}

impl Triangle {
    /// Creates a triangle with canonical per-triangle UVs: (0,0), (1,0), (0,1).
    pub fn new(vertices: [[f32; 3]; 3]) -> Result<Self, String> {
        Self::new_with_uvs(vertices, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])
    }

    /// Creates a triangle with explicit per-vertex texture coordinates.
    pub fn new_with_uvs(vertices: [[f32; 3]; 3], uvs: [[f32; 2]; 3]) -> Result<Self, String> {
        if vertices
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err("triangle vertices must contain only finite values".into());
        }
        if uvs.into_iter().flatten().any(|value| !value.is_finite()) {
            return Err("triangle texture coordinates must contain only finite values".into());
        }
        let flat_normal = triangle_flat_normal(vertices);
        Ok(Self {
            vertices,
            uvs,
            flat_normal,
        })
    }

    pub fn vertices(&self) -> &[[f32; 3]; 3] {
        &self.vertices
    }

    pub fn uvs(&self) -> &[[f32; 2]; 3] {
        &self.uvs
    }

    pub(crate) fn flat_normal(&self) -> Option<[f32; 3]> {
        self.flat_normal
    }
}

fn triangle_flat_normal(vertices: [[f32; 3]; 3]) -> Option<[f32; 3]> {
    let vertices = vertices.map(|vertex| vertex.map(f64::from));
    let edge_a = std::array::from_fn(|axis| vertices[1][axis] - vertices[0][axis]);
    let edge_b = std::array::from_fn(|axis| vertices[2][axis] - vertices[0][axis]);
    let normal = cross3(edge_a, edge_b);
    let length = dot3(normal, normal).sqrt();
    if length == 0.0 || !length.is_finite() {
        return None;
    }
    Some(normal.map(|value| (value / length) as f32))
}

/// A collection of triangles uploaded to the hardware ray-tracing backend.
#[derive(Clone, Debug)]
pub struct Scene {
    triangles: Vec<Triangle>,
    bounds: ([f32; 3], [f32; 3]),
}

impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.triangles == other.triangles
    }
}

impl Scene {
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.is_empty() {
            return Err("ray-tracing scenes must contain at least one triangle".into());
        }
        let mut minimum = [f32::INFINITY; 3];
        let mut maximum = [f32::NEG_INFINITY; 3];
        for vertex in triangles.iter().flat_map(|triangle| triangle.vertices()) {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(vertex[axis]);
                maximum[axis] = maximum[axis].max(vertex[axis]);
            }
        }
        Ok(Self {
            triangles,
            bounds: (minimum, maximum),
        })
    }

    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }

    pub(crate) fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        self.bounds
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
    pub(crate) fn from_backend(backend: NucleusRenderer, width: u32, height: u32) -> Self {
        Self {
            backend,
            width,
            height,
        }
    }

    /// Returns whether a Vulkan device exposes the required KHR hardware features.
    pub fn is_hardware_supported() -> bool {
        hardware_ray_tracing_supported()
    }

    /// Creates a standalone offscreen target with its own Vulkan device.
    /// Use `Renderer::rtx_renderer` to share the presentation device.
    pub fn new_standalone(width: u32, height: u32) -> Result<Self, String> {
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
        let backend = NucleusRenderer::new_standalone(width, height)?;
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

#[cfg(test)]
mod tests {
    use super::{Camera, Scene, Triangle};

    #[test]
    fn scene_bounds_cover_all_vertices() {
        let scene = Scene::new(vec![
            Triangle::new([[-1.0, -2.0, 2.0], [1.0, -2.0, 2.0], [0.0, 1.0, 2.0]]).unwrap(),
            Triangle::new([[3.0, 0.0, 4.0], [4.0, 0.0, 4.0], [3.0, 1.0, 4.0]]).unwrap(),
        ])
        .unwrap();

        assert_eq!(scene.bounds(), ([-1.0, -2.0, 2.0], [4.0, 1.0, 4.0]));
    }

    #[test]
    fn scene_equality_ignores_cached_bounds() {
        let scene = Scene::new(vec![Triangle::new([
            [-1.0, -2.0, 2.0],
            [1.0, -2.0, 2.0],
            [0.0, 1.0, 2.0],
        ])
        .unwrap()])
        .unwrap();
        let mut same_geometry = scene.clone();
        same_geometry.bounds = ([0.0; 3], [0.0; 3]);
        assert_eq!(scene, same_geometry);
    }

    #[test]
    fn camera_bounds_culling_handles_each_plane_and_rotated_cameras() {
        let camera = Camera::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            90.0,
            0.1,
            100.0,
        )
        .unwrap();
        let projection = camera.projection(32, 32).unwrap();
        let outside_vertices = [
            [[-0.5, -0.5, 0.05], [0.5, -0.5, 0.05], [0.0, 0.5, 0.05]],
            [[-0.5, -0.5, 101.0], [0.5, -0.5, 101.0], [0.0, 0.5, 101.0]],
            [[5.0, 0.0, 2.0], [6.0, 0.0, 2.0], [5.0, 1.0, 2.0]],
            [[-6.0, 0.0, 2.0], [-5.0, 0.0, 2.0], [-5.5, 1.0, 2.0]],
            [[0.0, 5.0, 2.0], [1.0, 5.0, 2.0], [0.5, 6.0, 2.0]],
            [[0.0, -6.0, 2.0], [1.0, -6.0, 2.0], [0.5, -5.0, 2.0]],
        ];
        let crossing = Scene::new(vec![
            Triangle::new([[-0.5, -0.5, 2.0], [0.5, -0.5, 2.0], [0.0, 0.5, 2.0]]).unwrap(),
            Triangle::new([[10.0, 0.0, 2.0], [11.0, 0.0, 2.0], [10.0, 1.0, 2.0]]).unwrap(),
        ])
        .unwrap();

        for vertices in outside_vertices {
            let outside = Scene::new(vec![Triangle::new(vertices).unwrap()]).unwrap();
            assert!(
                projection.bounds_outside_frustum(outside.bounds()),
                "scene should be outside the frustum: {vertices:?}"
            );
        }
        assert!(!projection.bounds_outside_frustum(crossing.bounds()));

        let rotated_camera = Camera::new(
            [10.0, 0.0, -4.0],
            [11.0, 0.0, -4.0],
            [0.0, 1.0, 0.0],
            90.0,
            0.01,
            100.0,
        )
        .unwrap();
        let rotated_projection = rotated_camera.projection(32, 32).unwrap();
        let rotated_outside = Scene::new(vec![Triangle::new([
            [12.0, -1.0, -9.0],
            [12.0, 1.0, -9.0],
            [13.0, 0.0, -9.0],
        ])
        .unwrap()])
        .unwrap();
        let rotated_visible = Scene::new(vec![Triangle::new([
            [12.0, -0.5, -4.0],
            [12.0, 0.5, -4.0],
            [12.0, 0.0, -3.5],
        ])
        .unwrap()])
        .unwrap();
        assert!(rotated_projection.bounds_outside_frustum(rotated_outside.bounds()));
        assert!(!rotated_projection.bounds_outside_frustum(rotated_visible.bounds()));
    }

    #[test]
    fn triangle_exposes_derived_flat_normal_and_keeps_geometry_equality() {
        let triangle = Triangle::new([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]]).unwrap();

        assert_eq!(triangle.flat_normal(), Some([0.0, 0.0, 1.0]));
        let mut same_geometry = triangle;
        same_geometry.flat_normal = None;
        assert_eq!(triangle, same_geometry);
    }

    #[test]
    fn degenerate_triangle_remains_constructible_without_a_flat_normal() {
        let triangle = Triangle::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]).unwrap();

        assert_eq!(triangle.flat_normal(), None);
    }

    #[test]
    fn camera_projects_world_vertices_to_screen_and_depth() {
        let camera = Camera::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            90.0,
            0.01,
            10_000.0,
        )
        .unwrap();
        let (triangles, count) = camera
            .projection(32, 32)
            .unwrap()
            .project_triangle_clipped(&[[-0.75, -0.75, 2.0], [0.75, -0.75, 2.0], [0.0, 0.75, 2.0]])
            .unwrap();
        assert_eq!(count, 1);
        let projected = triangles[0];

        assert!((projected[0][0] - 10.0).abs() < 0.001);
        assert!((projected[0][1] - 22.0).abs() < 0.001);
        assert!((projected[0][2] - 0.995_001).abs() < 0.001);
        assert!((projected[2][0] - 16.0).abs() < 0.001);
        assert!((projected[2][1] - 10.0).abs() < 0.001);
    }

    #[test]
    fn camera_clips_to_view_space_for_gpu_projection() {
        let camera = Camera::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            90.0,
            0.01,
            10_000.0,
        )
        .unwrap();
        let (triangles, count) = camera
            .projection(32, 32)
            .unwrap()
            .camera_space_triangle_clipped(&[
                [-0.75, -0.75, 2.0],
                [0.75, -0.75, 2.0],
                [0.0, 0.75, 2.0],
            ])
            .unwrap();

        assert_eq!(count, 1);
        assert!((triangles[0][0][0] + 0.75).abs() < 0.001);
        assert!((triangles[0][0][1] + 0.75).abs() < 0.001);
        assert!((triangles[0][0][2] - 2.0).abs() < 0.001);
        assert!((triangles[0][2][0]).abs() < 0.001);
        assert!((triangles[0][2][1] - 0.75).abs() < 0.001);
    }

    #[test]
    fn camera_clips_triangles_to_the_frustum_and_rejects_empty_viewports() {
        let camera = Camera::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            90.0,
            0.01,
            10_000.0,
        )
        .unwrap();
        let projection = camera.projection(32, 32).unwrap();
        let (clipped, count) = projection
            .project_triangle_clipped(&[[0.0, 0.0, -1.0], [1.0, 0.0, 2.0], [0.0, 1.0, 2.0]])
            .unwrap();
        assert_eq!(count, 3);
        assert!(
            clipped[..count]
                .iter()
                .flatten()
                .flatten()
                .all(|coordinate| coordinate.is_finite())
        );
        let (_, count) = projection
            .project_triangle_clipped(&[[0.0, 0.0, -3.0], [1.0, 0.0, -2.0], [0.0, 1.0, -4.0]])
            .unwrap();
        assert_eq!(count, 0);
        let (_, count) = projection
            .project_triangle_clipped(&[[10.0, -1.0, 2.0], [12.0, -1.0, 2.0], [11.0, 1.0, 2.0]])
            .unwrap();
        assert_eq!(count, 0);
        assert!(camera.projection(0, 32).is_err());
    }

    #[test]
    fn camera_rejects_triangles_beyond_the_far_plane() {
        let camera = Camera::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            90.0,
            0.01,
            10_000.0,
        )
        .unwrap();
        let (_, count) = camera
            .projection(32, 32)
            .unwrap()
            .project_triangle_clipped(&[
                [-1.0, -1.0, 10_001.0],
                [1.0, -1.0, 10_001.0],
                [0.0, 1.0, 10_001.0],
            ])
            .unwrap();

        assert_eq!(count, 0);

        let (_, count) = camera
            .projection(32, 32)
            .unwrap()
            .project_triangle_clipped(&[
                [-1.0, -1.0, 9_999.0],
                [1.0, -1.0, 9_999.0],
                [0.0, 1.0, 10_001.0],
            ])
            .unwrap();
        assert_eq!(count, 2, "the far plane clips crossing triangles");
    }

    #[test]
    fn camera_uses_configured_near_and_far_planes() {
        let camera = Camera::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            90.0,
            1.0,
            5.0,
        )
        .unwrap();
        let projection = camera.projection(32, 32).unwrap();

        let (triangles, count) = projection
            .project_triangle_clipped(&[[-1.0, -1.0, 2.0], [1.0, -1.0, 2.0], [0.0, 1.0, 2.0]])
            .unwrap();
        assert_eq!(count, 1);
        assert!((triangles[0][0][2] - 0.625).abs() < 0.001);

        let (_, count) = projection
            .project_triangle_clipped(&[[-1.0, -1.0, 6.0], [1.0, -1.0, 6.0], [0.0, 1.0, 6.0]])
            .unwrap();
        assert_eq!(count, 0);

        let (_, count) = projection
            .project_triangle_clipped(&[[-1.0, -1.0, 0.5], [1.0, -1.0, 0.5], [0.0, 1.0, 0.5]])
            .unwrap();
        assert_eq!(count, 0);
    }
}
