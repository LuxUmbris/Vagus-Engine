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

use std::collections::HashMap;

use crate::api::math::{Vec2, Vec3};
use crate::api::physics::PhysicsBody;
use crate::api::window::Window;

struct SceneMeshBatch {
    mesh_index: usize,
    texture: Option<crate::api::texture::Texture>,
    transforms: Vec<crate::api::scene::default::Transform3D>,
    flat_normals_valid: bool,
    gpu_instance_transforms_valid: bool,
}

#[derive(Default)]
struct SceneMeshBufferScratch {
    vertices: Vec<nucleus::GpuMeshVertex>,
    instances: Vec<nucleus::GpuInstanceTransform>,
}

#[derive(Default)]
struct SceneMeshBufferPool {
    available: Vec<SceneMeshBufferScratch>,
}

impl SceneMeshBufferPool {
    fn acquire(&mut self) -> SceneMeshBufferScratch {
        self.available.pop().unwrap_or_default()
    }

    fn recycle(&mut self, mut buffers: SceneMeshBufferScratch) {
        buffers.vertices.clear();
        buffers.instances.clear();
        self.available.push(buffers);
    }

    fn reserve_returns(&mut self, pending: usize, requested: usize) -> Result<(), String> {
        let additional = pending
            .checked_add(requested.saturating_sub(self.available.len()))
            .ok_or_else(|| "scene mesh buffer pool size overflow".to_owned())?;
        self.available
            .try_reserve(additional)
            .map_err(|error| format!("reserving scene mesh buffer pool failed: {error}"))
    }
}

#[derive(Default)]
struct SceneBatchScratch {
    mesh_batches: Vec<SceneMeshBatch>,
    mesh_lookup: HashMap<u64, Vec<usize>>,
    object_mesh_batches: HashMap<usize, usize>,
    mesh_buffer_pool: SceneMeshBufferPool,
    active_batches: usize,
}

impl SceneBatchScratch {
    fn reset(&mut self) {
        self.active_batches = 0;
        self.object_mesh_batches.clear();
        for candidates in self.mesh_lookup.values_mut() {
            candidates.clear();
        }
        for batch in &mut self.mesh_batches {
            batch.texture = None;
            batch.transforms.clear();
            batch.flat_normals_valid = false;
            batch.gpu_instance_transforms_valid = false;
        }
    }

    fn retain_active_mesh_keys(&mut self) {
        self.mesh_lookup
            .retain(|_, candidates| !candidates.is_empty());
    }
}

#[cfg(test)]
mod scene_batch_scratch_tests {
    use super::{SceneBatchScratch, SceneMeshBatch};
    use crate::api::scene::default::Transform3D;
    use crate::api::texture::Texture;

    #[test]
    fn scene_batch_scratch_reset_keeps_active_data() {
        let texture = Texture::from_rgba8(1, 1, &[255, 255, 255, 255]).unwrap();
        let mut transforms = Vec::with_capacity(4);
        transforms.push(Transform3D::default());
        let mut candidates = Vec::with_capacity(4);
        candidates.push(0);
        let mut scratch = SceneBatchScratch::default();
        scratch.mesh_batches.push(SceneMeshBatch {
            mesh_index: 0,
            texture: Some(texture),
            transforms,
            flat_normals_valid: true,
            gpu_instance_transforms_valid: true,
        });
        scratch.mesh_lookup.insert(7, candidates);
        scratch.object_mesh_batches.insert(0, 0);
        scratch.active_batches = 1;

        let batch_capacity = scratch.mesh_batches.capacity();
        let transform_capacity = scratch.mesh_batches[0].transforms.capacity();
        let candidate_capacity = scratch.mesh_lookup[&7].capacity();

        scratch.reset();

        assert_eq!(scratch.active_batches, 0);
        assert_eq!(scratch.mesh_batches.capacity(), batch_capacity);
        assert!(scratch.mesh_batches[0].texture.is_none());
        assert!(scratch.mesh_batches[0].transforms.is_empty());
        assert!(!scratch.mesh_batches[0].gpu_instance_transforms_valid);
        assert_eq!(
            scratch.mesh_batches[0].transforms.capacity(),
            transform_capacity
        );
        assert!(scratch.mesh_lookup[&7].is_empty());
        assert_eq!(scratch.mesh_lookup[&7].capacity(), candidate_capacity);
        assert!(scratch.object_mesh_batches.is_empty());

        scratch.mesh_lookup.get_mut(&7).unwrap().push(0);
        scratch.retain_active_mesh_keys();
        assert_eq!(scratch.mesh_lookup[&7].capacity(), candidate_capacity);
        scratch.reset();
        scratch.retain_active_mesh_keys();
        assert!(!scratch.mesh_lookup.contains_key(&7));
    }

    #[test]
    fn scene_mesh_gpu_buffers_reuse_storage_after_recycling() {
        let mut scratch = SceneBatchScratch::default();
        let mut buffers = scratch.mesh_buffer_pool.acquire();
        buffers.vertices.reserve(3);
        buffers.instances.reserve(2);
        buffers.vertices.push(nucleus::GpuMeshVertex {
            position: [0.0; 3],
            color: [255; 4],
            uv: [0.0; 2],
            normal: [0.0, 0.0, 1.0],
            light_direction: [0.0, 0.0, -1.0],
            ambient: 1.0,
        });
        buffers.instances.push(
            nucleus::GpuInstanceTransform::new(
                [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
                [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                ],
            )
            .unwrap(),
        );
        let vertex_pointer = buffers.vertices.as_ptr();
        let vertex_capacity = buffers.vertices.capacity();
        let instance_pointer = buffers.instances.as_ptr();
        let instance_capacity = buffers.instances.capacity();

        scratch.mesh_buffer_pool.recycle(buffers);
        let reused = scratch.mesh_buffer_pool.acquire();

        assert!(reused.vertices.is_empty());
        assert_eq!(reused.vertices.as_ptr(), vertex_pointer);
        assert_eq!(reused.vertices.capacity(), vertex_capacity);
        assert!(reused.instances.is_empty());
        assert_eq!(reused.instances.as_ptr(), instance_pointer);
        assert_eq!(reused.instances.capacity(), instance_capacity);
    }
}

/// An opaque RGB color used by the immediate-mode renderer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(red, green, blue)
    }
}

/// An 8-bit RGBA color for GPU primitives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgbaColor(pub u8, pub u8, pub u8, pub u8);

impl RgbaColor {
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self(red, green, blue, alpha)
    }
}

impl From<Color> for RgbaColor {
    fn from(Color(red, green, blue): Color) -> Self {
        Self(red, green, blue, u8::MAX)
    }
}

/// Common named colors, convertible to their 8-bit RGB values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedColor {
    Black,
    White,
    Red,
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
    Orange,
    Purple,
    Gray,
    LightGray,
    DarkGray,
}

impl From<NamedColor> for Color {
    fn from(color: NamedColor) -> Self {
        match color {
            NamedColor::Black => Self(0, 0, 0),
            NamedColor::White => Self(255, 255, 255),
            NamedColor::Red => Self(255, 0, 0),
            NamedColor::Green => Self(0, 255, 0),
            NamedColor::Blue => Self(0, 0, 255),
            NamedColor::Yellow => Self(255, 255, 0),
            NamedColor::Cyan => Self(0, 255, 255),
            NamedColor::Magenta => Self(255, 0, 255),
            NamedColor::Orange => Self(255, 165, 0),
            NamedColor::Purple => Self(128, 0, 128),
            NamedColor::Gray => Self(128, 128, 128),
            NamedColor::LightGray => Self(211, 211, 211),
            NamedColor::DarkGray => Self(64, 64, 64),
        }
    }
}

impl From<NamedColor> for RgbaColor {
    fn from(color: NamedColor) -> Self {
        Self::from(Color::from(color))
    }
}

#[cfg(test)]
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

#[cfg(test)]
impl Canvas {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let count = usize::try_from(width)
            .ok()
            .and_then(|width| usize::try_from(height).ok()?.checked_mul(width))
            .ok_or_else(|| "render target dimensions overflow address space".to_owned())?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|error| format!("could not allocate render target: {error}"))?;
        pixels.resize(count, Color::default());
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn clear(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    #[allow(dead_code)]
    pub fn pixel(&self, x: u32, y: u32) -> Color {
        self.pixels[(y * self.width + x) as usize]
    }

    fn dot_pixels(&mut self, x: i32, y: i32, color: Color) {
        self.set_pixel(x, y, color);
    }

    fn dot(&mut self, point: Vec2, color: Color) {
        if let Some((x, y)) = pixel_point(point) {
            self.dot_pixels(x, y, color);
        }
    }

    fn line_pixels(&mut self, start: (i32, i32), end: (i32, i32), color: Color) {
        let Some(((mut x0, mut y0), (x1, y1))) = self.clip_line(start, end) else {
            return;
        };
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.set_pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice_error = 2 * error;
            if twice_error >= dy {
                error += dy;
                x0 += sx;
            }
            if twice_error <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    fn line(&mut self, start: Vec2, end: Vec2, color: Color) {
        if let (Some(start), Some(end)) = (pixel_point(start), pixel_point(end)) {
            self.line_pixels(start, end, color);
        }
    }

    fn triangle_pixels(&mut self, a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
        let area = edge(a, b, c.0 as i64, c.1 as i64);
        if area == 0 {
            self.line_pixels(a, b, color);
            self.line_pixels(b, c, color);
            self.line_pixels(c, a, color);
            return;
        }
        let min_x = a.0.min(b.0).min(c.0).max(0) as u32;
        let min_y = a.1.min(b.1).min(c.1).max(0) as u32;
        let max_x = a.0.max(b.0).max(c.0).min(self.width as i32 - 1);
        let max_y = a.1.max(b.1).max(c.1).min(self.height as i32 - 1);
        if max_x < min_x as i32 || max_y < min_y as i32 {
            return;
        }
        for y in min_y..=max_y as u32 {
            for x in min_x..=max_x as u32 {
                let (x, y) = (x as i64, y as i64);
                let e0 = edge(a, b, x, y);
                let e1 = edge(b, c, x, y);
                let e2 = edge(c, a, x, y);
                if (area > 0 && e0 >= 0 && e1 >= 0 && e2 >= 0)
                    || (area < 0 && e0 <= 0 && e1 <= 0 && e2 <= 0)
                {
                    self.set_pixel(x as i32, y as i32, color);
                }
            }
        }
    }

    fn triangle(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        if let (Some(a), Some(b), Some(c)) = (pixel_point(a), pixel_point(b), pixel_point(c)) {
            self.triangle_pixels(a, b, c, color);
        }
    }

    fn triangle_outline_pixels(
        &mut self,
        a: (i32, i32),
        b: (i32, i32),
        c: (i32, i32),
        color: Color,
    ) {
        self.line_pixels(a, b, color);
        self.line_pixels(b, c, color);
        self.line_pixels(c, a, color);
    }

    fn triangle_outline(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        if let (Some(a), Some(b), Some(c)) = (pixel_point(a), pixel_point(b), pixel_point(c)) {
            self.triangle_outline_pixels(a, b, c, color);
        }
    }

    #[cfg(test)]
    fn rectangle_pixels(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        if width == 0 || height == 0 {
            return;
        }
        let left = (x as i64).max(0);
        let top = (y as i64).max(0);
        let right = (x as i64 + width as i64).min(self.width as i64);
        let bottom = (y as i64 + height as i64).min(self.height as i64);
        for py in top..bottom {
            for px in left..right {
                self.set_pixel_i64(px, py, color);
            }
        }
    }

    #[cfg(test)]
    fn rectangle(&mut self, position: Vec2, width: u32, height: u32, color: Color) {
        if let Some((x, y)) = pixel_point(position) {
            self.rectangle_pixels(x, y, width, height, color);
        }
    }

    fn rectangle_outline_pixels(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        if width == 0 || height == 0 {
            return;
        }
        let left = x as i64;
        let top = y as i64;
        let right = left + width as i64 - 1;
        let bottom = top + height as i64 - 1;
        let visible_left = left.max(0);
        let visible_right = right.min(self.width as i64 - 1);
        for px in visible_left..=visible_right {
            self.set_pixel_i64(px, top, color);
            self.set_pixel_i64(px, bottom, color);
        }
        let visible_top = (top + 1).max(0);
        let visible_bottom = (bottom - 1).min(self.height as i64 - 1);
        for py in visible_top..=visible_bottom {
            self.set_pixel_i64(left, py, color);
            self.set_pixel_i64(right, py, color);
        }
    }

    fn rectangle_outline(&mut self, position: Vec2, width: u32, height: u32, color: Color) {
        if let Some((x, y)) = pixel_point(position) {
            self.rectangle_outline_pixels(x, y, width, height, color);
        }
    }

    fn square_pixels(&mut self, x: i32, y: i32, size: u32, color: Color) {
        if size == 0 || self.width == 0 || self.height == 0 {
            return;
        }
        let left = (x as i64).max(0);
        let top = (y as i64).max(0);
        let right = (x as i64 + size as i64).min(self.width as i64);
        let bottom = (y as i64 + size as i64).min(self.height as i64);
        for py in top..bottom {
            for px in left..right {
                self.set_pixel(px as i32, py as i32, color);
            }
        }
    }

    fn square(&mut self, position: Vec2, size: u32, color: Color) {
        if let Some((x, y)) = pixel_point(position) {
            self.square_pixels(x, y, size, color);
        }
    }

    fn circle_pixels(&mut self, center: (i32, i32), radius: u32, color: Color) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (cx, cy) = (center.0 as i64, center.1 as i64);
        let radius = radius as i64;
        let left = (cx - radius).max(0);
        let right = (cx + radius).min(self.width as i64 - 1);
        let top = (cy - radius).max(0);
        let bottom = (cy + radius).min(self.height as i64 - 1);
        let radius_sq = (radius as i128) * (radius as i128);
        for y in top..=bottom {
            for x in left..=right {
                let dx = x - cx;
                let dy = y - cy;
                if (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128) <= radius_sq {
                    self.set_pixel(x as i32, y as i32, color);
                }
            }
        }
    }

    pub fn circle(&mut self, center: Vec2, radius: u32, color: Color) {
        if let Some(center) = pixel_point(center) {
            self.circle_pixels(center, radius, color);
        }
    }

    fn circle_outline_pixels(&mut self, center: (i32, i32), radius: u32, color: Color) {
        if radius == 0 {
            self.set_pixel(center.0, center.1, color);
            return;
        }
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (cx, cy) = (center.0 as i64, center.1 as i64);
        let radius = radius as i64;
        let outer_sq = (radius as i128) * (radius as i128);
        let inner = radius - 1;
        let inner_sq = (inner as i128) * (inner as i128);
        let left = (cx - radius).max(0);
        let right = (cx + radius).min(self.width as i64 - 1);
        let top = (cy - radius).max(0);
        let bottom = (cy + radius).min(self.height as i64 - 1);
        for y in top..=bottom {
            for x in left..=right {
                let dx = x - cx;
                let dy = y - cy;
                let distance_sq = (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128);
                if distance_sq <= outer_sq && distance_sq > inner_sq {
                    self.set_pixel_i64(x, y, color);
                }
            }
        }
    }

    fn circle_outline(&mut self, center: Vec2, radius: u32, color: Color) {
        if let Some(center) = pixel_point(center) {
            self.circle_outline_pixels(center, radius, color);
        }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        self.set_pixel_i64(x as i64, y as i64, color);
    }

    pub fn set_pixel_i64(&mut self, x: i64, y: i64, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        self.pixels[y as usize * self.width as usize + x as usize] = color;
    }

    pub fn clip_line(
        &self,
        start: (i32, i32),
        end: (i32, i32),
    ) -> Option<((i32, i32), (i32, i32))> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let (mut x0, mut y0) = (start.0 as f64, start.1 as f64);
        let (mut x1, mut y1) = (end.0 as f64, end.1 as f64);
        let (max_x, max_y) = (self.width as f64 - 1.0, self.height as f64 - 1.0);
        for _ in 0..8 {
            let c0 = out_code(x0, y0, max_x, max_y);
            let c1 = out_code(x1, y1, max_x, max_y);
            if c0 | c1 == 0 {
                return Some((
                    (x0.round() as i32, y0.round() as i32),
                    (x1.round() as i32, y1.round() as i32),
                ));
            }
            if c0 & c1 != 0 {
                return None;
            }
            let code = if c0 != 0 { c0 } else { c1 };
            let (x, y) = if code & 8 != 0 {
                (x0 + (x1 - x0) * (max_y - y0) / (y1 - y0), max_y)
            } else if code & 4 != 0 {
                (x0 + (x1 - x0) * (0.0 - y0) / (y1 - y0), 0.0)
            } else if code & 2 != 0 {
                (max_x, y0 + (y1 - y0) * (max_x - x0) / (x1 - x0))
            } else {
                (0.0, y0 + (y1 - y0) * (0.0 - x0) / (x1 - x0))
            };
            if code == c0 {
                (x0, y0) = (x, y);
            } else {
                (x1, y1) = (x, y);
            }
        }
        None
    }
}

fn pixel_point(point: Vec2) -> Option<(i32, i32)> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    let x = f64::from(point.x.round());
    let y = f64::from(point.y.round());
    if x < i32::MIN as f64 || x > i32::MAX as f64 || y < i32::MIN as f64 || y > i32::MAX as f64 {
        return None;
    }
    Some((x as i32, y as i32))
}

fn scene_sprite_rect(bounds: [(f64, f64); 3]) -> Option<(Vec2, u32, u32)> {
    let (left, right) = bounds[0];
    let (top, bottom) = bounds[1];
    if !left.is_finite()
        || !right.is_finite()
        || !top.is_finite()
        || !bottom.is_finite()
        || right <= left
        || bottom <= top
    {
        return None;
    }

    let left = left.floor();
    let right = right.ceil();
    let top = top.floor();
    let bottom = bottom.ceil();
    let width = right - left;
    let height = bottom - top;
    if left < f64::from(i32::MIN)
        || right > f64::from(i32::MAX)
        || top < f64::from(i32::MIN)
        || bottom > f64::from(i32::MAX)
        || width <= 0.0
        || height <= 0.0
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return None;
    }

    let position = Vec2 {
        x: left as f32,
        y: top as f32,
    };
    pixel_point(position)?;
    Some((position, width as u32, height as u32))
}

#[cfg(test)]
fn edge(a: (i32, i32), b: (i32, i32), x: i64, y: i64) -> i128 {
    (b.0 as i128 - a.0 as i128) * (y as i128 - a.1 as i128)
        - (b.1 as i128 - a.1 as i128) * (x as i128 - a.0 as i128)
}

#[cfg(test)]
fn out_code(x: f64, y: f64, max_x: f64, max_y: f64) -> u8 {
    (if x < 0.0 {
        1
    } else if x > max_x {
        2
    } else {
        0
    }) | (if y < 0.0 {
        4
    } else if y > max_y {
        8
    } else {
        0
    })
}

/// A high-level drawing context attached to one native window.
pub struct Renderer<'window> {
    window: &'window mut Window,
    clear_color: Color,
    presenter: nucleus::Renderer,
    gpu_triangles: Vec<nucleus::GpuTriangle>,
    gpu_opaque_triangles: Vec<nucleus::GpuTriangle>,
    gpu_transparent_triangles: Vec<nucleus::GpuTriangle>,
    projected_scene_scratch: Vec<nucleus::GpuTriangle>,
    gpu_texture_quads: Vec<nucleus::GpuTextureQuad>,
    game_scene_texture_scratch: Vec<nucleus::GpuTextureQuad>,
    game_scene_texture_cache:
        HashMap<crate::api::texture::Texture, crate::api::texture::GpuTexture>,
    scene_batch_scratch: SceneBatchScratch,
}

fn reset_projected_scene_scratch(
    scratch: &mut Vec<nucleus::GpuTriangle>,
    initial_capacity: usize,
) -> Result<(), String> {
    scratch.clear();
    scratch
        .try_reserve(initial_capacity)
        .map_err(|error| format!("reserving projected scene triangles failed: {error}"))
}

fn recycle_instanced_mesh_buffers(
    pool: &mut SceneMeshBufferPool,
    quads: &mut Vec<nucleus::GpuTextureQuad>,
) {
    for quad in quads.iter_mut() {
        if let Some((vertices, instances)) = quad.take_instanced_mesh_buffers() {
            pool.recycle(SceneMeshBufferScratch {
                vertices,
                instances,
            });
        }
    }
    quads.clear();
}

const CIRCLE_SEGMENTS: usize = 32;
const SCENE_BOUNDS_CULL_MIN_TRIANGLES: usize = 4;
const ECS_MESH_BOUNDS_CULL_MIN_TRIANGLES: usize = 8;

fn mesh_bounds_outside_frustum(
    projection: &crate::api::rtx::CameraProjection,
    mesh: &crate::api::scene::default::Mesh3D,
    transform: crate::api::scene::default::Transform3D,
) -> bool {
    mesh.bounds().is_some_and(|bounds| {
        transform
            .matrix()
            .transform_bounds(bounds)
            .is_ok_and(|world_bounds| projection.bounds_outside_frustum(world_bounds))
    })
}

fn circle_unit_vertices() -> &'static [[f32; 2]; CIRCLE_SEGMENTS] {
    static VERTICES: std::sync::OnceLock<[[f32; 2]; CIRCLE_SEGMENTS]> = std::sync::OnceLock::new();
    VERTICES.get_or_init(|| {
        std::array::from_fn(|segment| {
            let angle = std::f32::consts::TAU * segment as f32 / CIRCLE_SEGMENTS as f32;
            [angle.cos(), angle.sin()]
        })
    })
}

fn circle_outline_unit_vertices() -> &'static [[f64; 2]; CIRCLE_SEGMENTS] {
    static VERTICES: std::sync::OnceLock<[[f64; 2]; CIRCLE_SEGMENTS]> = std::sync::OnceLock::new();
    VERTICES.get_or_init(|| {
        std::array::from_fn(|segment| {
            let angle = std::f64::consts::TAU * segment as f64 / CIRCLE_SEGMENTS as f64;
            [angle.cos(), angle.sin()]
        })
    })
}

fn circle_outline_points(center: [f64; 2], radius: f64) -> [[f32; 2]; CIRCLE_SEGMENTS] {
    let units = circle_outline_unit_vertices();
    std::array::from_fn(|segment| {
        [
            (center[0] + radius * units[segment][0]) as f32,
            (center[1] + radius * units[segment][1]) as f32,
        ]
    })
}

impl<'window> Renderer<'window> {
    /// Returns the number of instanced Vulkan draws recorded in the latest presented frame.
    pub fn last_frame_instanced_draw_count(&self) -> usize {
        self.presenter.last_frame_instanced_draw_count()
    }

    pub fn new(window: &'window mut Window) -> Result<Self, String> {
        let (width, height) = window.framebuffer_size;
        let presenter = nucleus::Renderer::new(&window.backend, width, height)?;
        Ok(Self {
            window,
            clear_color: Color::default(),
            presenter,
            gpu_triangles: Vec::new(),
            gpu_opaque_triangles: Vec::new(),
            gpu_transparent_triangles: Vec::new(),
            projected_scene_scratch: Vec::new(),
            gpu_texture_quads: Vec::new(),
            game_scene_texture_scratch: Vec::new(),
            game_scene_texture_cache: HashMap::new(),
            scene_batch_scratch: SceneBatchScratch::default(),
        })
    }

    /// Creates a shader loader that shares this renderer's Vulkan device.
    pub fn shader_device(&self) -> crate::api::shader::ShaderDevice {
        crate::api::shader::ShaderDevice::from_shared(self.presenter.shader_device())
    }

    /// Rebuilds this renderer's graphics pipelines with a caller-loaded fragment shader.
    /// The module must come from this renderer's shared shader device and write color at
    /// location 0 using interfaces compatible with the installed vertex stage.
    pub fn set_fragment_shader(
        &mut self,
        module: &crate::api::shader::ShaderModule,
    ) -> Result<(), String> {
        if module.stage() != crate::api::shader::ShaderStage::Fragment {
            return Err("renderer fragment shader must use the fragment stage".into());
        }
        let device = self.presenter.shader_device();
        if !module.belongs_to_device(&device) {
            return Err("fragment shader module uses a different Vulkan device".into());
        }
        self.presenter
            .set_fragment_shader(module.words(), module.entry_point())
    }

    /// Rebuilds this renderer's graphics pipelines with a caller-loaded vertex shader.
    /// The module must come from this renderer's shared shader device, use the renderer's fixed
    /// vertex-input layout, and emit outputs compatible with the installed fragment stage.
    pub fn set_vertex_shader(
        &mut self,
        module: &crate::api::shader::ShaderModule,
    ) -> Result<(), String> {
        if module.stage() != crate::api::shader::ShaderStage::Vertex {
            return Err("renderer vertex shader must use the vertex stage".into());
        }
        let device = self.presenter.shader_device();
        if !module.belongs_to_device(&device) {
            return Err("vertex shader module uses a different Vulkan device".into());
        }
        self.presenter
            .set_vertex_shader(module.words(), module.entry_point())
    }

    /// Creates a texture uploader backed by this renderer's Vulkan device.
    pub fn texture_device(&self) -> Result<crate::api::texture::TextureDevice, String> {
        self.presenter
            .texture_device()
            .map(crate::api::texture::TextureDevice::from_backend)
    }

    /// Reports whether this renderer's selected Vulkan device supports KHR ray tracing.
    pub fn ray_tracing_supported(&self) -> bool {
        self.presenter.ray_tracing_supported()
    }

    /// Creates an offscreen ray tracer on this renderer's Vulkan device.
    pub fn rtx_renderer(
        &self,
        width: u32,
        height: u32,
    ) -> Result<crate::api::rtx::RtxRenderer, String> {
        let backend = self.presenter.rtx_renderer(width, height)?;
        Ok(crate::api::rtx::RtxRenderer::from_backend(
            backend, width, height,
        ))
    }

    pub fn window_mut(&mut self) -> &mut Window {
        self.window
    }

    pub fn clear(&mut self, color: impl Into<Color>) {
        self.gpu_triangles.clear();
        self.gpu_opaque_triangles.clear();
        self.gpu_transparent_triangles.clear();
        recycle_instanced_mesh_buffers(
            &mut self.scene_batch_scratch.mesh_buffer_pool,
            &mut self.gpu_texture_quads,
        );
        recycle_instanced_mesh_buffers(
            &mut self.scene_batch_scratch.mesh_buffer_pool,
            &mut self.game_scene_texture_scratch,
        );
        self.clear_color = color.into();
    }

    /// Draws a point at rounded `Vec2` coordinates; invalid coordinates are ignored.
    pub fn draw_dot(&mut self, point: Vec2, color: impl Into<Color>) {
        self.draw_rectangle_gpu(point, 1, 1, color);
    }

    /// Draws between rounded `Vec2` endpoints; invalid endpoints are ignored.
    pub fn draw_line(&mut self, start: Vec2, end: Vec2, color: impl Into<Color>) {
        self.draw_line_gpu(start, end, color);
    }

    /// Draws a filled triangle from rounded `Vec2` vertices.
    pub fn draw_triangle(&mut self, a: Vec2, b: Vec2, c: Vec2, color: impl Into<Color>) {
        self.draw_triangle_gpu(a, b, c, color);
    }

    /// Draws a filled triangle in the ordered GPU 2D primitive batch.
    pub fn draw_triangle_gpu(&mut self, a: Vec2, b: Vec2, c: Vec2, color: impl Into<Color>) {
        self.draw_triangle_gpu_rgba(a, b, c, RgbaColor::from(color.into()));
    }

    /// Draws a filled RGBA triangle on the GPU layer.
    pub fn draw_triangle_gpu_rgba(
        &mut self,
        a: Vec2,
        b: Vec2,
        c: Vec2,
        color: impl Into<RgbaColor>,
    ) {
        if let (Some(a), Some(b), Some(c)) = (pixel_point(a), pixel_point(b), pixel_point(c)) {
            let RgbaColor(red, green, blue, alpha) = color.into();
            self.gpu_triangles.push(nucleus::GpuTriangle::new_rgba(
                [
                    [a.0 as f32, a.1 as f32],
                    [b.0 as f32, b.1 as f32],
                    [c.0 as f32, c.1 as f32],
                ],
                [red, green, blue, alpha],
            ));
        }
    }

    /// Queues a GPU-sampled texture quad behind GPU geometry for the next present.
    /// Invalid positions and zero-sized rectangles are ignored.
    pub fn draw_texture_gpu(
        &mut self,
        texture: &crate::api::texture::GpuTexture,
        position: Vec2,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let Some((x, y)) = pixel_point(position) else {
            return Ok(());
        };
        if width == 0 || height == 0 {
            return Ok(());
        }
        let backend_texture = texture.backend_clone();
        self.presenter.validate_gpu_texture(&backend_texture)?;
        let quad = nucleus::GpuTextureQuad::new(
            backend_texture,
            [x as f32, y as f32],
            [width as f32, height as f32],
        )?;
        self.gpu_texture_quads
            .try_reserve(1)
            .map_err(|error| format!("queueing GPU texture quad failed: {error}"))?;
        self.gpu_texture_quads.push(quad);
        Ok(())
    }

    /// Queues 2D ECS sprites using each physics hitbox's axis-aligned bounds.
    /// Hitbox coordinates are interpreted as framebuffer pixels; CPU textures are cached by exact
    /// contents until this renderer is dropped. Objects without finite, positive bounds are skipped.
    pub fn draw_game_scene_2d(
        &mut self,
        scene: &crate::api::scene::default::GameScene2D,
    ) -> Result<usize, String> {
        let texture_device = self.texture_device()?;
        let mut queued = 0;
        scene.try_for_each_object::<String>(|texture, physics_object| {
            let Some((position, width, height)) = physics_object
                .broadphase_bounds()
                .and_then(scene_sprite_rect)
            else {
                return Ok(());
            };

            let gpu_texture = if let Some(texture) = self.game_scene_texture_cache.get(texture) {
                texture.clone()
            } else {
                self.game_scene_texture_cache
                    .try_reserve(1)
                    .map_err(|error| format!("reserving scene texture cache failed: {error}"))?;
                let gpu_texture = texture_device.upload(texture)?;
                self.game_scene_texture_cache
                    .insert(texture.clone(), gpu_texture.clone());
                gpu_texture
            };
            self.draw_texture_gpu(&gpu_texture, position, width, height)?;
            queued += 1;
            Ok(())
        })?;
        Ok(queued)
    }

    /// Queues sprites from the active 2D scene held by `manager`.
    /// Returns an error when no scene is selected.
    pub fn draw_current_scene_2d(
        &mut self,
        manager: &crate::api::scene::SceneManager<crate::api::scene::default::GameScene2D, Vec2>,
    ) -> Result<usize, String> {
        let scene = manager
            .current_scene()
            .ok_or_else(|| "scene manager has no selected 2D scene".to_owned())?;
        self.draw_game_scene_2d(scene)
    }

    /// Queues meshes from the active 3D scene held by `manager`.
    /// Returns an error when no scene is selected.
    pub fn draw_current_scene_3d(
        &mut self,
        manager: &crate::api::scene::SceneManager<crate::api::scene::default::GameScene3D, Vec3>,
        camera: &crate::api::rtx::Camera,
        color: impl Into<RgbaColor>,
        light_direction: [f32; 3],
        ambient: f32,
    ) -> Result<usize, String> {
        let scene = manager
            .current_scene()
            .ok_or_else(|| "scene manager has no selected 3D scene".to_owned())?;
        self.draw_game_scene_3d(scene, camera, color, light_direction, ambient)
    }

    /// Queues 3D ECS meshes with per-object material textures and per-vertex UVs.
    /// The supplied color tints sampled texture values; physics hitboxes remain independent.
    pub fn draw_game_scene_3d(
        &mut self,
        scene: &crate::api::scene::default::GameScene3D,
        camera: &crate::api::rtx::Camera,
        color: impl Into<RgbaColor>,
        light_direction: [f32; 3],
        ambient: f32,
    ) -> Result<usize, String> {
        recycle_instanced_mesh_buffers(
            &mut self.scene_batch_scratch.mesh_buffer_pool,
            &mut self.game_scene_texture_scratch,
        );
        let (width, height) = self.window.framebuffer_size;
        let projection = camera.projection(width, height)?;
        let gpu_projection = projection.gpu_projection_parameters()?;
        let gpu_camera_projection = projection.gpu_camera_projection()?;
        let RgbaColor(red, green, blue, alpha) = color.into();
        if !ambient.is_finite() || !(0.0..=1.0).contains(&ambient) {
            return Err("ambient intensity must be between zero and one".into());
        }
        let light_length = light_direction
            .iter()
            .map(|value| f64::from(*value) * f64::from(*value))
            .sum::<f64>()
            .sqrt();
        if !light_length.is_finite()
            || light_length == 0.0
            || light_direction.iter().any(|value| !value.is_finite())
        {
            return Err("directional light vector must be finite and nonzero".into());
        }
        let light_direction = light_direction.map(|value| (f64::from(value) / light_length) as f32);
        {
            let scratch = &mut self.scene_batch_scratch;
            scratch.reset();
            scene.try_for_each_mesh::<String>(|mesh_index, material, mesh, transform| {
                if alpha != u8::MAX
                    || !material.texture().is_fully_opaque()
                    || mesh.triangles().is_empty()
                {
                    return Ok(());
                }
                let fingerprint = mesh.fingerprint();
                let existing = scratch
                    .mesh_lookup
                    .get(&fingerprint)
                    .and_then(|candidates| {
                        candidates.iter().copied().find(|&index| {
                            scene
                                .mesh_at(scratch.mesh_batches[index].mesh_index)
                                .is_some_and(|candidate| candidate == mesh)
                                && scratch.mesh_batches[index]
                                    .texture
                                    .as_ref()
                                    .is_some_and(|texture| texture == material.texture())
                        })
                    });
                if let Some(index) = existing {
                    scratch
                        .object_mesh_batches
                        .try_reserve(1)
                        .map_err(|error| {
                            format!("reserving scene object batch lookup failed: {error}")
                        })?;
                    scratch.mesh_batches[index].transforms.push(transform);
                    scratch.object_mesh_batches.insert(mesh_index, index);
                } else {
                    let batch_index = scratch.active_batches;
                    let candidates = scratch.mesh_lookup.entry(fingerprint).or_default();
                    candidates
                        .try_reserve(1)
                        .map_err(|error| format!("reserving scene mesh lookup failed: {error}"))?;
                    scratch
                        .mesh_batches
                        .try_reserve(usize::from(batch_index == scratch.mesh_batches.len()))
                        .map_err(|error| format!("reserving scene mesh batches failed: {error}"))?;
                    candidates.push(batch_index);
                    if batch_index == scratch.mesh_batches.len() {
                        let mut transforms = Vec::new();
                        transforms.try_reserve(1).map_err(|error| {
                            format!("reserving scene mesh transforms failed: {error}")
                        })?;
                        transforms.push(transform);
                        scratch.mesh_batches.push(SceneMeshBatch {
                            mesh_index,
                            texture: Some(material.texture().clone()),
                            transforms,
                            flat_normals_valid: false,
                            gpu_instance_transforms_valid: false,
                        });
                    } else {
                        let batch = &mut scratch.mesh_batches[batch_index];
                        batch.mesh_index = mesh_index;
                        batch.texture = Some(material.texture().clone());
                        batch.transforms.push(transform);
                    }
                    scratch
                        .object_mesh_batches
                        .try_reserve(1)
                        .map_err(|error| {
                            format!("reserving scene object batch lookup failed: {error}")
                        })?;
                    scratch.object_mesh_batches.insert(mesh_index, batch_index);
                    scratch.active_batches += 1;
                }
                Ok(())
            })?;
            scratch.retain_active_mesh_keys();
            for batch in &mut scratch.mesh_batches[..scratch.active_batches] {
                if batch.transforms.len() > 1 {
                    let mesh = scene
                        .mesh_at(batch.mesh_index)
                        .ok_or_else(|| "scene mesh batch index became invalid".to_owned())?;
                    batch.flat_normals_valid = mesh.flat_normals()?.is_some();
                }
                batch.gpu_instance_transforms_valid = batch.transforms.len() > 1
                    && batch.flat_normals_valid
                    && alpha == u8::MAX
                    && batch
                        .texture
                        .as_ref()
                        .is_some_and(|texture| texture.is_fully_opaque())
                    && batch
                        .transforms
                        .iter()
                        .all(|transform| transform.matrix().gpu_instance().is_ok());
            }
        }
        let texture_device = self.texture_device()?;
        let active_mesh_batches =
            &self.scene_batch_scratch.mesh_batches[..self.scene_batch_scratch.active_batches];
        let requested_mesh_buffers = active_mesh_batches
            .iter()
            .filter(|batch| {
                batch.transforms.len() > 1
                    && batch.flat_normals_valid
                    && alpha == u8::MAX
                    && batch
                        .texture
                        .as_ref()
                        .is_some_and(|texture| texture.is_fully_opaque())
            })
            .count();
        let pending_mesh_buffers = self
            .gpu_texture_quads
            .iter()
            .chain(&self.game_scene_texture_scratch)
            .filter(|quad| quad.is_instanced_mesh())
            .count();
        self.scene_batch_scratch
            .mesh_buffer_pool
            .reserve_returns(pending_mesh_buffers, requested_mesh_buffers)?;

        let batch_scratch = &mut self.scene_batch_scratch;
        let mesh_batches = &batch_scratch.mesh_batches[..batch_scratch.active_batches];
        let object_mesh_batches = &batch_scratch.object_mesh_batches;
        let mesh_buffer_pool = &mut batch_scratch.mesh_buffer_pool;
        let (scene_quads, texture_cache) = (
            &mut self.game_scene_texture_scratch,
            &mut self.game_scene_texture_cache,
        );
        scene.try_for_each_mesh::<String>(|mesh_index, material, mesh, transform| {
            let texture = material.texture();
            let batch_index = object_mesh_batches.get(&mesh_index).copied();
            let is_instanced_batch =
                batch_index.is_some_and(|index| mesh_batches[index].gpu_instance_transforms_valid);
            if !is_instanced_batch
                && mesh.triangles().len() >= ECS_MESH_BOUNDS_CULL_MIN_TRIANGLES
                && mesh_bounds_outside_frustum(&projection, mesh, transform)
            {
                return Ok(());
            }
            if is_instanced_batch {
                return Ok(());
            }
            let gpu_texture = if let Some(texture) = texture_cache.get(texture) {
                texture.clone()
            } else {
                texture_cache
                    .try_reserve(1)
                    .map_err(|error| format!("reserving scene texture cache failed: {error}"))?;
                let uploaded = texture_device.upload(texture)?;
                texture_cache.insert(texture.clone(), uploaded.clone());
                uploaded
            };
            let texture_is_opaque = texture.is_fully_opaque();
            let transform = transform.matrix();
            scene_quads
                .try_reserve(mesh.triangles().len())
                .map_err(|error| format!("reserving textured scene triangles failed: {error}"))?;
            for triangle in mesh.triangles() {
                let mut world_points = [[0.0; 3]; 3];
                for (output, point) in world_points.iter_mut().zip(triangle.vertices()) {
                    *output = transform.transform_point(*point)?;
                }
                let points = projection.camera_space_triangle(&world_points)?;
                if projection.triangle_outside_frustum(&points) {
                    continue;
                }
                let vertices = world_points.map(|point| point.map(f64::from));
                let edge_a: [f64; 3] =
                    std::array::from_fn(|axis| vertices[1][axis] - vertices[0][axis]);
                let edge_b: [f64; 3] =
                    std::array::from_fn(|axis| vertices[2][axis] - vertices[0][axis]);
                let normal = [
                    edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
                    edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
                    edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
                ];
                let normal_length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
                if normal_length == 0.0 || !normal_length.is_finite() {
                    return Err("flat lighting requires non-degenerate triangles".into());
                }
                let normal = normal.map(|value| (value / normal_length) as f32);
                scene_quads.push(nucleus::GpuTextureQuad::new_camera_triangle(
                    gpu_texture.backend_clone(),
                    points,
                    *triangle.uvs(),
                    gpu_projection,
                    [red, green, blue, alpha],
                    Some((normal, light_direction, ambient)),
                    texture_is_opaque && alpha == u8::MAX,
                )?);
            }
            Ok(())
        })?;
        let mut count = scene_quads.len();
        for batch in mesh_batches {
            let batch_texture = batch
                .texture
                .as_ref()
                .expect("active scene mesh batches retain their texture");
            if batch.transforms.len() < 2
                || !batch.flat_normals_valid
                || !batch.gpu_instance_transforms_valid
                || alpha != u8::MAX
                || !batch_texture.is_fully_opaque()
            {
                continue;
            }
            let batch_mesh = scene
                .mesh_at(batch.mesh_index)
                .ok_or_else(|| "scene mesh batch index became invalid".to_owned())?;
            let gpu_texture = if let Some(texture) = texture_cache.get(batch_texture) {
                texture.clone()
            } else {
                texture_cache
                    .try_reserve(1)
                    .map_err(|error| format!("reserving scene texture cache failed: {error}"))?;
                let uploaded = texture_device.upload(batch_texture)?;
                texture_cache.insert(batch_texture.clone(), uploaded.clone());
                uploaded
            };
            let mut mesh_buffers = mesh_buffer_pool.acquire();
            mesh_buffers.instances.clear();
            mesh_buffers
                .instances
                .try_reserve(batch.transforms.len())
                .map_err(|error| format!("reserving GPU scene instances failed: {error}"))?;
            let mut visible_triangles = 0usize;
            let mut fallback_instance = None;
            for transform in &batch.transforms {
                let matrix = transform.matrix();
                if batch_mesh.triangles().len() >= ECS_MESH_BOUNDS_CULL_MIN_TRIANGLES
                    && batch_mesh.bounds().is_some_and(|bounds| {
                        matrix.transform_bounds(bounds).is_ok_and(|world_bounds| {
                            projection.bounds_outside_frustum(world_bounds)
                        })
                    })
                {
                    fallback_instance.get_or_insert(*transform);
                    continue;
                }
                let mut instance_visible = false;
                for triangle in batch_mesh.triangles() {
                    let mut world_points = [[0.0; 3]; 3];
                    for (output, point) in world_points.iter_mut().zip(triangle.vertices()) {
                        *output = matrix.transform_point(*point)?;
                    }
                    let camera_points = projection.camera_space_triangle(&world_points)?;
                    if !projection.triangle_outside_frustum(&camera_points) {
                        visible_triangles = visible_triangles
                            .checked_add(1)
                            .ok_or_else(|| "visible scene triangle count overflow".to_owned())?;
                        instance_visible = true;
                    }
                }
                if instance_visible {
                    mesh_buffers.instances.push(matrix.gpu_instance()?);
                } else {
                    fallback_instance.get_or_insert(*transform);
                }
            }
            if mesh_buffers.instances.is_empty() {
                mesh_buffer_pool.recycle(mesh_buffers);
                continue;
            }
            if mesh_buffers.instances.len() == 1 {
                if let Some(transform) = fallback_instance {
                    mesh_buffers
                        .instances
                        .push(transform.matrix().gpu_instance()?);
                }
            }
            let flat_normals = batch_mesh
                .flat_normals()?
                .expect("instanced mesh batches have validated flat normals");
            let vertex_count = batch_mesh
                .triangles()
                .len()
                .checked_mul(3)
                .ok_or_else(|| "GPU scene mesh vertex count overflow".to_owned())?;
            mesh_buffers.vertices.clear();
            mesh_buffers
                .vertices
                .try_reserve(vertex_count)
                .map_err(|error| format!("reserving GPU scene mesh vertices failed: {error}"))?;
            for (triangle, normal) in batch_mesh.triangles().iter().zip(flat_normals) {
                for index in 0..3 {
                    mesh_buffers.vertices.push(nucleus::GpuMeshVertex {
                        position: triangle.vertices()[index],
                        color: [red, green, blue, alpha],
                        uv: triangle.uvs()[index],
                        normal: *normal,
                        light_direction,
                        ambient,
                    });
                }
            }
            scene_quads.push(nucleus::GpuTextureQuad::new_instanced_camera_mesh(
                gpu_texture.backend_clone(),
                std::mem::take(&mut mesh_buffers.vertices),
                std::mem::take(&mut mesh_buffers.instances),
                gpu_camera_projection,
                true,
            )?);
            count = count
                .checked_add(visible_triangles)
                .ok_or_else(|| "visible scene triangle count overflow".to_owned())?;
        }
        self.gpu_texture_quads
            .try_reserve(scene_quads.len())
            .map_err(|error| format!("queueing textured scene triangles failed: {error}"))?;
        scene_quads.sort_by(|left, right| {
            if left.is_instanced_mesh() != right.is_instanced_mesh() {
                return right.is_instanced_mesh().cmp(&left.is_instanced_mesh());
            }
            right
                .sort_depth()
                .unwrap_or(f32::NEG_INFINITY)
                .total_cmp(&left.sort_depth().unwrap_or(f32::NEG_INFINITY))
        });
        self.gpu_texture_quads.extend(scene_quads.drain(..));
        Ok(count)
    }

    /// Drops renderer-owned CPU-to-GPU scene texture cache entries.
    /// Textures already queued or referenced by Vulkan descriptors may live until those references retire.
    pub fn clear_game_scene_texture_cache(&mut self) {
        self.game_scene_texture_cache.clear();
    }

    /// Draws a one-pixel-wide line as two triangles in the GPU batch.
    /// Degenerate lines draw one pixel; non-finite or unrepresentable endpoints are ignored.
    pub fn draw_line_gpu(&mut self, start: Vec2, end: Vec2, color: impl Into<Color>) {
        self.draw_line_gpu_rgba(start, end, RgbaColor::from(color.into()));
    }

    /// Draws an RGBA one-pixel-wide line as two GPU triangles.
    pub fn draw_line_gpu_rgba(&mut self, start: Vec2, end: Vec2, color: impl Into<RgbaColor>) {
        let (Some(start), Some(end)) = (pixel_point(start), pixel_point(end)) else {
            return;
        };
        let dx = f64::from(end.0) - f64::from(start.0);
        let dy = f64::from(end.1) - f64::from(start.1);
        let length = dx.hypot(dy);
        if length == 0.0 {
            self.draw_rectangle_gpu_rgba(
                Vec2 {
                    x: start.0 as f32,
                    y: start.1 as f32,
                },
                1,
                1,
                color,
            );
            return;
        }

        let offset = [-dy / length * 0.5, dx / length * 0.5];
        let start = [f64::from(start.0) + 0.5, f64::from(start.1) + 0.5];
        let end = [f64::from(end.0) + 0.5, f64::from(end.1) + 0.5];
        let start_left = [start[0] + offset[0], start[1] + offset[1]];
        let start_right = [start[0] - offset[0], start[1] - offset[1]];
        let end_left = [end[0] + offset[0], end[1] + offset[1]];
        let end_right = [end[0] - offset[0], end[1] - offset[1]];
        let to_gpu = |point: [f64; 2]| point.map(|coordinate| coordinate as f32);
        let RgbaColor(red, green, blue, alpha) = color.into();
        let color = [red, green, blue, alpha];
        self.gpu_triangles.reserve(2);
        self.gpu_triangles.extend([
            nucleus::GpuTriangle::new_rgba(
                [to_gpu(start_left), to_gpu(end_left), to_gpu(end_right)],
                color,
            ),
            nucleus::GpuTriangle::new_rgba(
                [to_gpu(start_left), to_gpu(end_right), to_gpu(start_right)],
                color,
            ),
        ]);
    }

    /// Draws a filled pixel-aligned rectangle as two triangles in the GPU batch.
    /// Zero-sized or unrepresentable rectangles are ignored.
    pub fn draw_rectangle_gpu(
        &mut self,
        position: Vec2,
        width: u32,
        height: u32,
        color: impl Into<Color>,
    ) {
        self.draw_rectangle_gpu_rgba(position, width, height, RgbaColor::from(color.into()));
    }

    /// Draws an RGBA pixel-aligned rectangle as two triangles in the GPU batch.
    pub fn draw_rectangle_gpu_rgba(
        &mut self,
        position: Vec2,
        width: u32,
        height: u32,
        color: impl Into<RgbaColor>,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let Some((x, y)) = pixel_point(position) else {
            return;
        };
        let right = i64::from(x) + i64::from(width);
        let bottom = i64::from(y) + i64::from(height);
        if right > i64::from(i32::MAX) || bottom > i64::from(i32::MAX) {
            return;
        }

        let left = x as f32;
        let top = y as f32;
        let right = right as f32;
        let bottom = bottom as f32;
        let RgbaColor(red, green, blue, alpha) = color.into();
        let color = [red, green, blue, alpha];
        self.gpu_triangles.extend([
            nucleus::GpuTriangle::new_rgba([[left, top], [right, top], [right, bottom]], color),
            nucleus::GpuTriangle::new_rgba([[left, top], [right, bottom], [left, bottom]], color),
        ]);
    }

    /// Draws a filled 32-segment circle fan in the GPU batch.
    /// Radius zero draws one pixel; circles outside representable coordinates are ignored.
    pub fn draw_circle_gpu(&mut self, center: Vec2, radius: u32, color: impl Into<Color>) {
        self.draw_circle_gpu_rgba(center, radius, RgbaColor::from(color.into()));
    }

    /// Draws an RGBA 32-segment circle fan in the GPU batch.
    pub fn draw_circle_gpu_rgba(&mut self, center: Vec2, radius: u32, color: impl Into<RgbaColor>) {
        let Some((x, y)) = pixel_point(center) else {
            return;
        };
        if radius == 0 {
            self.draw_rectangle_gpu_rgba(
                Vec2 {
                    x: x as f32,
                    y: y as f32,
                },
                1,
                1,
                color,
            );
            return;
        }

        let radius = i64::from(radius);
        let left = i64::from(x) - radius;
        let right = i64::from(x) + radius;
        let top = i64::from(y) - radius;
        let bottom = i64::from(y) + radius;
        if left < i64::from(i32::MIN)
            || right > i64::from(i32::MAX)
            || top < i64::from(i32::MIN)
            || bottom > i64::from(i32::MAX)
        {
            return;
        }

        let RgbaColor(red, green, blue, alpha) = color.into();
        let color = [red, green, blue, alpha];
        let radius = radius as f32;
        let center = [x as f32, y as f32];
        let unit_vertices = circle_unit_vertices();
        self.gpu_triangles.reserve(CIRCLE_SEGMENTS);
        for segment in 0..CIRCLE_SEGMENTS {
            let next_segment = (segment + 1) % CIRCLE_SEGMENTS;
            let point_at =
                |unit: [f32; 2]| [center[0] + radius * unit[0], center[1] + radius * unit[1]];
            self.gpu_triangles.push(nucleus::GpuTriangle::new_rgba(
                [
                    center,
                    point_at(unit_vertices[segment]),
                    point_at(unit_vertices[next_segment]),
                ],
                color,
            ));
        }
    }

    /// Draws a GPU triangle from pixel-space x/y and normalized Vulkan depth in [0, 1].
    pub fn draw_triangle_gpu_3d(
        &mut self,
        a: Vec3,
        b: Vec3,
        c: Vec3,
        color: impl Into<Color>,
    ) -> Result<(), String> {
        let Color(red, green, blue) = color.into();
        self.draw_triangle_gpu_3d_rgba(a, b, c, RgbaColor::rgba(red, green, blue, u8::MAX))
    }

    /// Draws an RGBA GPU triangle from pixel-space x/y and normalized Vulkan depth in [0, 1].
    /// Translucent triangles join the back-to-front scene transparency pass.
    pub fn draw_triangle_gpu_3d_rgba(
        &mut self,
        a: Vec3,
        b: Vec3,
        c: Vec3,
        color: impl Into<RgbaColor>,
    ) -> Result<(), String> {
        let vertices = [a, b, c];
        if vertices
            .iter()
            .any(|vertex| !vertex.z.is_finite() || !(0.0..=1.0).contains(&vertex.z))
        {
            return Err("GPU triangle depth must be finite and between zero and one".into());
        }
        let points = vertices.map(|vertex| {
            pixel_point(Vec2 {
                x: vertex.x,
                y: vertex.y,
            })
            .map(|(x, y)| [x as f32, y as f32, vertex.z])
        });
        let [Some(a), Some(b), Some(c)] = points else {
            return Err("GPU triangle screen coordinates must be finite and representable".into());
        };
        let RgbaColor(red, green, blue, alpha) = color.into();
        let target = if alpha == u8::MAX {
            &mut self.gpu_opaque_triangles
        } else {
            &mut self.gpu_transparent_triangles
        };
        target
            .try_reserve(1)
            .map_err(|error| format!("queueing GPU triangle failed: {error}"))?;
        target.push(nucleus::GpuTriangle::new_3d_rgba(
            [a, b, c],
            [red, green, blue, alpha],
            None,
        ));
        Ok(())
    }

    /// Queues world-space triangles for GPU projection with flat Lambert lighting.
    /// `light_direction` points toward the directional light and `ambient` is in `[0, 1]`.
    /// Vulkan clips the submitted camera-space triangles against the camera frustum.
    pub fn draw_scene_gpu(
        &mut self,
        scene: &crate::api::rtx::Scene,
        camera: &crate::api::rtx::Camera,
        color: impl Into<Color>,
        light_direction: [f32; 3],
        ambient: f32,
    ) -> Result<usize, String> {
        let Color(red, green, blue) = color.into();
        self.draw_scene_gpu_rgba(
            scene,
            camera,
            RgbaColor::rgba(red, green, blue, u8::MAX),
            light_direction,
            ambient,
        )
    }

    /// Queues a world-space scene for GPU projection and sorts translucent triangles back-to-front.
    /// `light_direction` points toward the directional light and `ambient` is in `[0, 1]`.
    /// Triangles wholly outside any frustum plane are culled before upload; Vulkan clips crossing triangles.
    /// The return value is the number of GPU triangles queued; extreme projected outliers use software clipping
    /// to avoid driver precision loss.
    /// Sorting uses each input triangle's mean camera depth; intersecting surfaces may still need geometry splitting.
    pub fn draw_scene_gpu_rgba(
        &mut self,
        scene: &crate::api::rtx::Scene,
        camera: &crate::api::rtx::Camera,
        color: impl Into<RgbaColor>,
        light_direction: [f32; 3],
        ambient: f32,
    ) -> Result<usize, String> {
        let (width, height) = self.window.framebuffer_size;
        let RgbaColor(red, green, blue, alpha) = color.into();
        reset_projected_scene_scratch(&mut self.projected_scene_scratch, 0)?;
        let projection = camera.projection(width, height)?;
        let gpu_projection = projection.gpu_projection_parameters()?;
        if !ambient.is_finite() || !(0.0..=1.0).contains(&ambient) {
            return Err("ambient intensity must be between zero and one".into());
        }
        let light_length = light_direction
            .iter()
            .map(|value| f64::from(*value) * f64::from(*value))
            .sum::<f64>()
            .sqrt();
        if !light_length.is_finite()
            || light_length == 0.0
            || light_direction.iter().any(|value| !value.is_finite())
        {
            return Err("directional light vector must be finite and nonzero".into());
        }
        let light_direction = light_direction.map(|value| (f64::from(value) / light_length) as f32);

        if scene.triangles().len() >= SCENE_BOUNDS_CULL_MIN_TRIANGLES
            && projection.bounds_outside_frustum(scene.bounds())
        {
            return Ok(0);
        }
        reset_projected_scene_scratch(&mut self.projected_scene_scratch, scene.triangles().len())?;

        for triangle in scene.triangles() {
            let points = projection.camera_space_triangle(triangle.vertices())?;
            if projection.triangle_outside_frustum(&points) {
                continue;
            }
            let normal = triangle
                .flat_normal()
                .ok_or_else(|| "flat lighting requires non-degenerate triangles".to_owned())?;
            // Keep pathological clip coordinates on the CPU safety path; some drivers lose huge triangles.
            let needs_software_clipping = points.iter().any(|point| {
                point[0].abs() / gpu_projection[0] > 1.0e6
                    || point[1].abs() / gpu_projection[1] > 1.0e6
                    || point[2].abs() > 1.0e6
            });
            if needs_software_clipping {
                let (clipped, count) =
                    projection.camera_space_triangle_clipped(triangle.vertices())?;
                self.projected_scene_scratch
                    .try_reserve(count)
                    .map_err(|error| {
                        format!("reserving projected scene triangles failed: {error}")
                    })?;
                for points in clipped[..count].iter().copied() {
                    let [depth_a, depth_b, depth_c] =
                        points.map(|point| projection.normalized_depth(point[2]));
                    let sort_depths = [depth_a?, depth_b?, depth_c?];
                    self.projected_scene_scratch
                        .push(nucleus::GpuTriangle::new_camera_space_rgba(
                            points,
                            gpu_projection,
                            sort_depths,
                            [red, green, blue, alpha],
                            Some((normal, light_direction, ambient)),
                        )?);
                }
            } else {
                let sort_depths = if alpha == u8::MAX {
                    [0.0; 3]
                } else {
                    let [depth_a, depth_b, depth_c] =
                        points.map(|point| projection.normalized_depth(point[2]));
                    [depth_a?, depth_b?, depth_c?]
                };
                self.projected_scene_scratch
                    .try_reserve(1)
                    .map_err(|error| {
                        format!("reserving projected scene triangles failed: {error}")
                    })?;
                self.projected_scene_scratch
                    .push(nucleus::GpuTriangle::new_camera_space_rgba(
                        points,
                        gpu_projection,
                        sort_depths,
                        [red, green, blue, alpha],
                        Some((normal, light_direction, ambient)),
                    )?);
            }
        }

        let target = if alpha == u8::MAX {
            &mut self.gpu_opaque_triangles
        } else {
            &mut self.gpu_transparent_triangles
        };
        target
            .try_reserve(self.projected_scene_scratch.len())
            .map_err(|error| format!("reserving GPU scene batch failed: {error}"))?;
        let count = self.projected_scene_scratch.len();
        target.append(&mut self.projected_scene_scratch);
        Ok(count)
    }

    /// Draws a triangle outline from rounded `Vec2` vertices.
    pub fn draw_triangle_outline(&mut self, a: Vec2, b: Vec2, c: Vec2, color: impl Into<Color>) {
        let color = color.into();
        self.draw_line_gpu(a, b, color);
        self.draw_line_gpu(b, c, color);
        self.draw_line_gpu(c, a, color);
    }

    pub fn draw_rectangle(
        &mut self,
        position: Vec2,
        width: u32,
        height: u32,
        color: impl Into<Color>,
    ) {
        self.draw_rectangle_gpu(position, width, height, color);
    }

    pub fn draw_rectangle_outline(
        &mut self,
        position: Vec2,
        width: u32,
        height: u32,
        color: impl Into<Color>,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let Some((left, top)) = pixel_point(position) else {
            return;
        };
        let right = i64::from(left) + i64::from(width) - 1;
        let bottom = i64::from(top) + i64::from(height) - 1;
        if right > i64::from(i32::MAX) || bottom > i64::from(i32::MAX) {
            return;
        }
        let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
        let color = color.into();
        self.draw_line_gpu(Vec2 { x: left, y: top }, Vec2 { x: right, y: top }, color);
        self.draw_line_gpu(
            Vec2 { x: right, y: top },
            Vec2 {
                x: right,
                y: bottom,
            },
            color,
        );
        self.draw_line_gpu(
            Vec2 {
                x: right,
                y: bottom,
            },
            Vec2 { x: left, y: bottom },
            color,
        );
        self.draw_line_gpu(Vec2 { x: left, y: bottom }, Vec2 { x: left, y: top }, color);
    }

    pub fn draw_square(&mut self, position: Vec2, size: u32, color: impl Into<Color>) {
        self.draw_rectangle_gpu(position, size, size, color);
    }

    /// Draws a circle at a rounded `Vec2` center.
    pub fn draw_circle(&mut self, center: Vec2, radius: u32, color: impl Into<Color>) {
        self.draw_circle_gpu(center, radius, color);
    }

    /// Draws a circle outline at a rounded `Vec2` center.
    pub fn draw_circle_outline(&mut self, center: Vec2, radius: u32, color: impl Into<Color>) {
        let Some((x, y)) = pixel_point(center) else {
            return;
        };
        if radius == 0 {
            self.draw_dot(
                Vec2 {
                    x: x as f32,
                    y: y as f32,
                },
                color,
            );
            return;
        }
        let radius = i64::from(radius);
        if i64::from(x) - radius < i64::from(i32::MIN)
            || i64::from(x) + radius > i64::from(i32::MAX)
            || i64::from(y) - radius < i64::from(i32::MIN)
            || i64::from(y) + radius > i64::from(i32::MAX)
        {
            return;
        }

        let points = circle_outline_points([f64::from(x), f64::from(y)], radius as f64);
        let color = color.into();
        self.gpu_triangles.reserve(CIRCLE_SEGMENTS * 2);
        for segment in 0..CIRCLE_SEGMENTS {
            let start = points[segment];
            let end = points[(segment + 1) % CIRCLE_SEGMENTS];
            self.draw_line_gpu(
                Vec2 {
                    x: start[0],
                    y: start[1],
                },
                Vec2 {
                    x: end[0],
                    y: end[1],
                },
                color,
            );
        }
    }

    pub fn present(&mut self) -> Result<(), String> {
        self.present_gpu(self.clear_color)
    }

    /// Clears the frame to `clear_color` and submits the queued GPU layers.
    pub fn present_gpu(&mut self, clear_color: impl Into<Color>) -> Result<(), String> {
        let Color(red, green, blue) = clear_color.into();
        let (width, height) = self.window.framebuffer_size;
        self.sort_transparent_triangles();
        self.presenter
            .present_gpu_layers_with_textures(
                width,
                height,
                [red, green, blue],
                &self.gpu_opaque_triangles,
                &self.gpu_transparent_triangles,
                &self.gpu_triangles,
                &self.gpu_texture_quads,
            )
            .map(|()| {
                self.gpu_triangles.clear();
                self.gpu_opaque_triangles.clear();
                self.gpu_transparent_triangles.clear();
                recycle_instanced_mesh_buffers(
                    &mut self.scene_batch_scratch.mesh_buffer_pool,
                    &mut self.gpu_texture_quads,
                );
                recycle_instanced_mesh_buffers(
                    &mut self.scene_batch_scratch.mesh_buffer_pool,
                    &mut self.game_scene_texture_scratch,
                );
            })
    }

    /// Presents queued GPU primitives and returns RGBA pixels when supported by the surface.
    pub fn present_gpu_readback(
        &mut self,
        clear_color: impl Into<Color>,
    ) -> Result<Vec<[u8; 4]>, String> {
        let Color(red, green, blue) = clear_color.into();
        let (width, height) = self.window.framebuffer_size;
        self.sort_transparent_triangles();
        self.presenter
            .present_gpu_readback_layers_with_textures(
                width,
                height,
                [red, green, blue],
                &self.gpu_opaque_triangles,
                &self.gpu_transparent_triangles,
                &self.gpu_triangles,
                &self.gpu_texture_quads,
            )
            .map(|pixels| {
                self.gpu_triangles.clear();
                self.gpu_opaque_triangles.clear();
                self.gpu_transparent_triangles.clear();
                recycle_instanced_mesh_buffers(
                    &mut self.scene_batch_scratch.mesh_buffer_pool,
                    &mut self.gpu_texture_quads,
                );
                recycle_instanced_mesh_buffers(
                    &mut self.scene_batch_scratch.mesh_buffer_pool,
                    &mut self.game_scene_texture_scratch,
                );
                pixels
            })
    }

    fn sort_transparent_triangles(&mut self) {
        self.gpu_transparent_triangles
            .sort_by(|left, right| right.average_depth().total_cmp(&left.average_depth()));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Canvas, Color, NamedColor, circle_outline_points, circle_outline_unit_vertices,
        circle_unit_vertices, reset_projected_scene_scratch,
    };
    use crate::api::math::Vec2;
    use crate::api::math::Vec3;
    use crate::api::rtx::Triangle;
    use crate::api::scene::default::{GameObject3D, GameScene3D, Mesh3D, Transform3D};
    use nucleus::GpuTriangle;

    #[test]
    fn named_colors_map_to_rgb_values() {
        assert_eq!(Color::from(NamedColor::Black), Color(0, 0, 0));
        assert_eq!(Color::from(NamedColor::White), Color(255, 255, 255));
        assert_eq!(Color::from(NamedColor::Red), Color(255, 0, 0));
        assert_eq!(Color::from(NamedColor::Blue), Color(0, 0, 255));
    }

    #[test]
    fn projected_scene_scratch_reuses_capacity_and_clears_previous_triangles() {
        let triangle = GpuTriangle::new_3d(
            [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [0.0, 1.0, 1.0]],
            [255, 0, 0],
        );
        let mut scratch = vec![triangle; 4];
        let capacity = scratch.capacity();

        reset_projected_scene_scratch(&mut scratch, 4).unwrap();

        assert!(scratch.is_empty());
        assert_eq!(scratch.capacity(), capacity);

        scratch.push(triangle);
        reset_projected_scene_scratch(&mut scratch, 0).unwrap();

        assert!(scratch.is_empty());
        assert_eq!(scratch.capacity(), capacity);
    }

    #[test]
    fn game_scene_texture_scratch_reuses_capacity_and_clears_stale_items() {
        let mut scratch = vec![1_u32, 2, 3, 4];
        let allocation = scratch.as_ptr();
        let capacity = scratch.capacity();

        scratch.clear();
        assert!(scratch.is_empty());
        assert_eq!(scratch.as_ptr(), allocation);
        assert_eq!(scratch.capacity(), capacity);

        scratch.extend([8, 9]);
        assert_eq!(scratch, [8, 9]);
        assert_eq!(scratch.as_ptr(), allocation);
        assert_eq!(scratch.capacity(), capacity);
        scratch.clear();
        assert!(scratch.is_empty());
        assert_eq!(scratch.as_ptr(), allocation);
        assert_eq!(scratch.capacity(), capacity);
    }

    #[test]
    fn game_scene_mesh_transform_matrix_matches_mesh_vertex_conversion() {
        let transform = Transform3D::new(
            Vec3 {
                x: 3.0,
                y: 4.0,
                z: 5.0,
            },
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: std::f32::consts::FRAC_PI_2,
            },
            Vec3 {
                x: 2.0,
                y: 1.0,
                z: 1.0,
            },
        )
        .unwrap();
        let matrix = transform.matrix();
        let expected = [[3.0, 6.0, 5.0], [2.0, 4.0, 5.0], [3.0, 4.0, 6.0]];
        for (point, expected) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
            .into_iter()
            .zip(expected)
        {
            let actual = matrix.transform_point(point).unwrap();
            for axis in 0..3 {
                assert!((actual[axis] - expected[axis]).abs() < 1.0e-6);
            }
        }

        let mut object = GameObject3D::default();
        object.set_mesh(Mesh3D::new(vec![
            Triangle::new([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
        ]));
        object.set_transform(transform);
        let mut scene = GameScene3D::with_capacity(1).unwrap();
        scene.objects_mut().push(object).unwrap();
        scene
            .try_for_each_mesh::<String>(|_, _, mesh, transform| {
                let matrix = transform.matrix();
                for (triangle, expected) in mesh.triangles().iter().zip([expected]) {
                    for (point, expected) in triangle.vertices().iter().zip(expected) {
                        let actual = matrix.transform_point(*point).unwrap();
                        for axis in 0..3 {
                            assert!((actual[axis] - expected[axis]).abs() < 1.0e-6);
                        }
                    }
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn circle_unit_vertices_are_cached_and_match_the_fan_geometry() {
        let vertices = circle_unit_vertices();
        assert!(std::ptr::eq(vertices, circle_unit_vertices()));
        assert_eq!(vertices.len(), 32);

        for (segment, vertex) in vertices.iter().enumerate() {
            let angle = std::f32::consts::TAU * segment as f32 / vertices.len() as f32;
            assert_eq!(*vertex, [angle.cos(), angle.sin()]);
        }
    }

    #[test]
    fn circle_outline_points_reuse_cached_angles_and_match_legacy_geometry() {
        let units = circle_outline_unit_vertices();
        assert!(std::ptr::eq(units, circle_outline_unit_vertices()));

        let center = [11.0, 17.0];
        let radius = 5.0;
        let points = circle_outline_points(center, radius);
        for (segment, point) in points.iter().enumerate() {
            let angle = std::f64::consts::TAU * segment as f64 / 32.0;
            assert_eq!(
                *point,
                [
                    (center[0] + radius * angle.cos()) as f32,
                    (center[1] + radius * angle.sin()) as f32,
                ]
            );
        }
    }

    #[test]
    fn software_canvas_draws_primitives_with_clipping() {
        let mut canvas = Canvas::new(12, 12).unwrap();
        let red = Color::from(NamedColor::Red);
        canvas.clear(Color(0, 0, 0));
        canvas.dot(Vec2 { x: 0.0, y: 0.0 }, red);
        canvas.line(Vec2 { x: 1.0, y: 2.0 }, Vec2 { x: 5.0, y: 2.0 }, red);
        canvas.triangle(
            Vec2 { x: 1.0, y: 4.0 },
            Vec2 { x: 5.0, y: 4.0 },
            Vec2 { x: 3.0, y: 8.0 },
            red,
        );
        canvas.square(Vec2 { x: 7.0, y: 1.0 }, 3, red);
        canvas.circle(Vec2 { x: 8.0, y: 8.0 }, 2, red);

        assert_eq!(canvas.pixel(0, 0), red);
        assert_eq!(canvas.pixel(3, 2), red);
        assert_eq!(canvas.pixel(3, 5), red);
        assert_eq!(canvas.pixel(8, 2), red);
        assert_eq!(canvas.pixel(8, 8), red);
        assert_eq!(canvas.pixel(11, 11), Color(0, 0, 0));
        canvas.dot(Vec2 { x: -1.0, y: 0.0 }, red);
        canvas.dot(Vec2 { x: 12.0, y: 0.0 }, red);
    }

    #[test]
    fn vec2_coordinates_round_to_pixels_and_ignore_invalid_points() {
        let mut canvas = Canvas::new(6, 6).unwrap();
        let red = Color::from(NamedColor::Red);
        canvas.dot(Vec2 { x: 1.6, y: 2.2 }, red);
        canvas.line(Vec2 { x: 0.0, y: 4.0 }, Vec2 { x: 2.9, y: 4.0 }, red);
        canvas.dot(
            Vec2 {
                x: f32::NAN,
                y: 0.0,
            },
            red,
        );
        canvas.dot(
            Vec2 {
                x: i32::MAX as f32,
                y: 0.0,
            },
            red,
        );

        assert_eq!(canvas.pixel(2, 2), red);
        assert_eq!(canvas.pixel(2, 4), red);
        assert_eq!(canvas.pixel(0, 0), Color::default());
    }

    #[test]
    fn rectangle_primitives_clip_and_preserve_the_requested_color() {
        let mut canvas = Canvas::new(8, 8).unwrap();
        let background = Color(0, 0, 0);
        let fill = Color(20, 80, 160);
        let outline = Color(200, 40, 10);
        canvas.clear(background);

        canvas.rectangle(Vec2 { x: -1.0, y: 1.0 }, 4, 3, fill);
        canvas.rectangle_outline(Vec2 { x: 4.0, y: 1.0 }, 4, 3, outline);

        assert_eq!(canvas.pixel(0, 1), fill);
        assert_eq!(canvas.pixel(2, 2), fill);
        assert_eq!(canvas.pixel(3, 3), background);
        assert_eq!(canvas.pixel(4, 1), outline);
        assert_eq!(canvas.pixel(7, 2), outline);
        assert_eq!(canvas.pixel(6, 2), background);
    }

    #[test]
    fn circle_and_triangle_outlines_use_the_passed_color() {
        let mut canvas = Canvas::new(10, 10).unwrap();
        let background = Color(0, 0, 0);
        let circle = Color(0, 180, 40);
        let triangle = Color(80, 30, 200);
        canvas.clear(background);

        canvas.circle_outline(Vec2 { x: 2.0, y: 2.0 }, 2, circle);
        canvas.triangle_outline(
            Vec2 { x: 5.0, y: 1.0 },
            Vec2 { x: 9.0, y: 1.0 },
            Vec2 { x: 7.0, y: 5.0 },
            triangle,
        );

        assert_eq!(canvas.pixel(2, 0), circle);
        assert_eq!(canvas.pixel(2, 2), background);
        assert_eq!(canvas.pixel(5, 1), triangle);
        assert_eq!(canvas.pixel(7, 5), triangle);
        assert_eq!(canvas.pixel(7, 2), background);
    }
}
