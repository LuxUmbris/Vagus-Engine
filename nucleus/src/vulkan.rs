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

use std::{
    ffi::c_void,
    sync::{Arc, Mutex, MutexGuard},
};

#[cfg(target_os = "linux")]
pub enum NativeWindow {
    Xlib {
        display: *mut c_void,
        window: u64,
    },
    Wayland {
        display: *mut c_void,
        surface: *mut c_void,
    },
}

#[cfg(target_os = "windows")]
pub enum NativeWindow {
    Win32 {
        instance: *mut c_void,
        window: *mut c_void,
    },
}

fn native_window(window: &crate::WindowHandle) -> NativeWindow {
    match &window.backend {
        #[cfg(target_os = "linux")]
        crate::WindowBackend::LinuxX11(window) => {
            let (display, window) = window.vulkan_handles();
            NativeWindow::Xlib { display, window }
        }
        #[cfg(target_os = "linux")]
        crate::WindowBackend::LinuxWayland(window) => NativeWindow::Wayland {
            display: window.display_ptr(),
            surface: window.surface_ptr(),
        },
        #[cfg(target_os = "windows")]
        crate::WindowBackend::Windows(window) => {
            let (instance, window) = window.vulkan_handles();
            NativeWindow::Win32 { instance, window }
        }
    }
}

pub(crate) struct VulkanRenderer {
    _entry: Option<ash::Entry>,
    instance: Option<ash::Instance>,
    surface_loader: Option<ash::khr::surface::Instance>,
    surface: ash::vk::SurfaceKHR,
    device: Option<ash::Device>,
    shared_device: Option<Arc<SharedDevice>>,
    swapchain_loader: Option<ash::khr::swapchain::Device>,
    swapchain: ash::vk::SwapchainKHR,
    physical_device: ash::vk::PhysicalDevice,
    queue_family: u32,
    ray_tracing_supported: bool,
    api_version: u32,
    queue: ash::vk::Queue,
    command_pool: ash::vk::CommandPool,
    command_buffer: ash::vk::CommandBuffer,
    image_available: ash::vk::Semaphore,
    render_finished: Vec<ash::vk::Semaphore>,
    fence: ash::vk::Fence,
    staging_buffer: ash::vk::Buffer,
    staging_memory: ash::vk::DeviceMemory,
    staging_mapped: *mut c_void,
    staging_size: ash::vk::DeviceSize,
    staging_memory_coherent: bool,
    rgba_scratch: Vec<u8>,
    triangle_vertex_buffer: ash::vk::Buffer,
    triangle_vertex_memory: ash::vk::DeviceMemory,
    triangle_vertex_mapped: *mut c_void,
    triangle_vertex_capacity: usize,
    triangle_vertex_scratch: Vec<GpuVertex>,
    texture_draw_range_scratch: Vec<std::ops::Range<usize>>,
    triangle_vertex_memory_coherent: bool,
    last_frame_instanced_draw_count: usize,
    instance_vertex_buffer: ash::vk::Buffer,
    instance_vertex_memory: ash::vk::DeviceMemory,
    instance_vertex_mapped: *mut c_void,
    instance_vertex_capacity: usize,
    instance_vertex_memory_coherent: bool,
    instance_data_scratch: Vec<GpuInstanceData>,
    texture_vertex_ranges: Vec<std::ops::Range<usize>>,
    texture_instance_ranges: Vec<Option<std::ops::Range<usize>>>,
    texture_set_layout: ash::vk::DescriptorSetLayout,
    texture_descriptor_pool: ash::vk::DescriptorPool,
    texture_descriptor_capacity: usize,
    texture_descriptor_sets: Vec<ash::vk::DescriptorSet>,
    texture_descriptor_textures: Vec<Option<crate::GpuTexture>>,
    default_texture_descriptor_pool: ash::vk::DescriptorPool,
    default_texture_descriptor_set: ash::vk::DescriptorSet,
    white_texture: Option<crate::GpuTexture>,
    in_flight_textures: Vec<crate::GpuTexture>,
    readback_supported: bool,
    extent: ash::vk::Extent2D,
    format: ash::vk::Format,
    swapchain_images: Vec<ash::vk::Image>,
    swapchain_image_views: Vec<ash::vk::ImageView>,
    framebuffers: Vec<ash::vk::Framebuffer>,
    render_pass: ash::vk::RenderPass,
    pipeline_layout: ash::vk::PipelineLayout,
    graphics_pipeline: ash::vk::Pipeline,
    transparent_pipeline: ash::vk::Pipeline,
    // Retained so swapchain recreation preserves caller-selected shader stages.
    custom_vertex_shader: Option<(Vec<u32>, String)>,
    custom_fragment_shader: Option<(Vec<u32>, String)>,
    depth_format: ash::vk::Format,
    depth_image: ash::vk::Image,
    depth_memory: ash::vk::DeviceMemory,
    depth_view: ash::vk::ImageView,
}

/// Camera transform and perspective parameters consumed by the vertex shader.
/// `origin.w` enables the world-to-camera transform; zero means vertices are already camera-space.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuCameraProjection {
    origin: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    forward: [f32; 4],
    parameters: [f32; 4],
}

impl GpuCameraProjection {
    /// Creates a view transform from an origin and orthonormal camera basis.
    /// Parameters are horizontal extent, vertical extent, near plane, and far plane.
    pub fn new(
        origin: [f32; 3],
        right: [f32; 3],
        up: [f32; 3],
        forward: [f32; 3],
        parameters: [f32; 4],
    ) -> Result<Self, String> {
        if origin
            .into_iter()
            .chain(right)
            .chain(up)
            .chain(forward)
            .chain(parameters)
            .any(|value| !value.is_finite())
        {
            return Err("GPU camera projection values must be finite".into());
        }
        if right == [0.0; 3]
            || up == [0.0; 3]
            || forward == [0.0; 3]
            || parameters[0] <= 0.0
            || parameters[1] <= 0.0
            || parameters[2] <= 0.0
            || parameters[3] <= parameters[2]
        {
            return Err("GPU camera projection has an invalid basis or frustum".into());
        }
        Ok(Self {
            origin: [origin[0], origin[1], origin[2], 1.0],
            right: [right[0], right[1], right[2], 0.0],
            up: [up[0], up[1], up[2], 0.0],
            forward: [forward[0], forward[1], forward[2], 0.0],
            parameters,
        })
    }

    fn camera_space(parameters: [f32; 4]) -> Result<Self, String> {
        let mut projection = Self::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            parameters,
        )?;
        projection.origin[3] = 0.0;
        Ok(projection)
    }

    fn as_bytes(&self) -> &[u8] {
        // SAFETY: repr(C) contains only five contiguous [f32; 4] arrays.
        unsafe {
            std::slice::from_raw_parts(
                std::ptr::from_ref(self).cast::<u8>(),
                std::mem::size_of::<Self>(),
            )
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GpuTriangle {
    points: [[f32; 3]; 3],
    color: [u8; 4],
    lighting: Option<FlatLighting>,
    camera_projection: Option<GpuCameraProjection>,
    sort_depths: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug)]
struct FlatLighting {
    normal: [f32; 3],
    direction: [f32; 3],
    ambient: f32,
}

#[derive(Clone)]
pub struct GpuTextureQuad {
    texture: crate::GpuTexture,
    position: [f32; 2],
    size: [f32; 2],
    camera_triangle: Option<CameraTextureTriangle>,
    instanced_mesh: Option<CameraTextureMesh>,
    opaque: bool,
}

#[derive(Clone)]
struct CameraTextureTriangle {
    points: [[f32; 3]; 3],
    uvs: [[f32; 2]; 3],
    projection: [f32; 4],
    sort_depth: f32,
    color: [u8; 4],
    lighting: Option<FlatLighting>,
}

/// One local-space vertex in an instanced textured mesh batch.
#[derive(Clone, Copy, Debug)]
pub struct GpuMeshVertex {
    pub position: [f32; 3],
    pub color: [u8; 4],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
    pub light_direction: [f32; 3],
    pub ambient: f32,
}

/// Per-instance model and inverse-transpose normal transforms, column-major.
#[derive(Clone, Copy, Debug)]
pub struct GpuInstanceTransform {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 3],
}

impl GpuInstanceTransform {
    /// Validates and stores a column-major model matrix and inverse-transpose normal matrix.
    pub fn new(model: [[f32; 4]; 4], normal: [[f32; 4]; 3]) -> Result<Self, String> {
        if model
            .into_iter()
            .flatten()
            .chain(normal.into_iter().flatten())
            .any(|value| !value.is_finite())
        {
            return Err("GPU instance transform values must be finite".into());
        }
        Ok(Self { model, normal })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct GpuInstanceData {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 3],
    enabled: [f32; 4],
}

#[derive(Clone)]
struct CameraTextureMesh {
    vertices: Vec<GpuMeshVertex>,
    instances: Vec<GpuInstanceTransform>,
    projection: GpuCameraProjection,
}

impl GpuTextureQuad {
    pub fn new(
        texture: crate::GpuTexture,
        position: [f32; 2],
        size: [f32; 2],
    ) -> Result<Self, String> {
        if !position.into_iter().chain(size).all(f32::is_finite)
            || size.into_iter().any(|extent| extent <= 0.0)
            || !(position[0] + size[0]).is_finite()
            || !(position[1] + size[1]).is_finite()
        {
            return Err("GPU texture quad requires finite coordinates and positive extents".into());
        }
        Ok(Self {
            texture,
            position,
            size,
            camera_triangle: None,
            instanced_mesh: None,
            opaque: false,
        })
    }

    pub fn new_camera_triangle(
        texture: crate::GpuTexture,
        points: [[f32; 3]; 3],
        uvs: [[f32; 2]; 3],
        projection: [f32; 4],
        color: [u8; 4],
        lighting: Option<([f32; 3], [f32; 3], f32)>,
        opaque: bool,
    ) -> Result<Self, String> {
        GpuCameraProjection::camera_space(projection)?;
        if points.into_iter().flatten().any(|value| !value.is_finite())
            || uvs.into_iter().flatten().any(|value| !value.is_finite())
        {
            return Err("GPU textured triangle coordinates must be finite".into());
        }
        let lighting = lighting.map(|(normal, direction, ambient)| FlatLighting {
            normal,
            direction,
            ambient,
        });
        if lighting.is_some_and(|lighting| {
            lighting
                .normal
                .into_iter()
                .chain(lighting.direction)
                .chain([lighting.ambient])
                .any(|value| !value.is_finite())
                || !(0.0..=1.0).contains(&lighting.ambient)
        }) {
            return Err("GPU textured triangle lighting is invalid".into());
        }
        let sort_depth = points.iter().map(|point| point[2]).sum::<f32>() / 3.0;
        Ok(Self {
            texture,
            position: [0.0; 2],
            size: [1.0; 2],
            camera_triangle: Some(CameraTextureTriangle {
                points,
                uvs,
                projection,
                sort_depth,
                color,
                lighting,
            }),
            instanced_mesh: None,
            opaque,
        })
    }

    /// Creates one textured mesh draw with per-instance model and normal transforms.
    pub fn new_instanced_camera_mesh(
        texture: crate::GpuTexture,
        vertices: Vec<GpuMeshVertex>,
        instances: Vec<GpuInstanceTransform>,
        projection: GpuCameraProjection,
        opaque: bool,
    ) -> Result<Self, String> {
        if vertices.is_empty()
            || vertices.len() % 3 != 0
            || instances.len() < 2
            || vertices.len() > u32::MAX as usize
            || instances.len() > u32::MAX as usize
        {
            return Err(
                "GPU instanced mesh requires a triangle list and 2..=u32::MAX instances".into(),
            );
        }
        if vertices.iter().any(|vertex| {
            vertex
                .position
                .into_iter()
                .chain(vertex.uv)
                .chain(vertex.normal)
                .chain(vertex.light_direction)
                .chain([vertex.ambient])
                .any(|value| !value.is_finite())
                || !(0.0..=1.0).contains(&vertex.ambient)
        }) {
            return Err("GPU instanced mesh vertex contains invalid values".into());
        }
        Ok(Self {
            texture,
            position: [0.0; 2],
            size: [1.0; 2],
            camera_triangle: None,
            instanced_mesh: Some(CameraTextureMesh {
                vertices,
                instances,
                projection,
            }),
            opaque,
        })
    }

    /// Camera-space depth for painter ordering; screen-space quads return `None`.
    pub fn sort_depth(&self) -> Option<f32> {
        self.camera_triangle
            .as_ref()
            .map(|triangle| triangle.sort_depth)
    }

    fn camera_projection(&self) -> Option<[f32; 4]> {
        self.camera_triangle
            .as_ref()
            .map(|triangle| triangle.projection)
    }

    /// Returns whether this draw contains a local-space mesh with per-instance transforms.
    pub fn is_instanced_mesh(&self) -> bool {
        self.instanced_mesh.is_some()
    }

    /// Moves CPU-side mesh staging buffers out of this queued draw for reuse after submission.
    /// The mesh remains present but has no vertices or instances until repopulated.
    pub fn take_instanced_mesh_buffers(
        &mut self,
    ) -> Option<(Vec<GpuMeshVertex>, Vec<GpuInstanceTransform>)> {
        let mesh = self.instanced_mesh.as_mut()?;
        Some((
            std::mem::take(&mut mesh.vertices),
            std::mem::take(&mut mesh.instances),
        ))
    }
}

fn texture_draw_ranges(
    ranges: &mut Vec<std::ops::Range<usize>>,
    count: usize,
    mut same_texture: impl FnMut(usize, usize) -> bool,
) -> Result<(), String> {
    ranges.clear();
    if count == 0 {
        return Ok(());
    }
    ranges
        .try_reserve(count)
        .map_err(|error| format!("reserving GPU texture draw ranges failed: {error}"))?;
    let mut start = 0;
    for index in 1..count {
        if !same_texture(index - 1, index) {
            ranges.push(start..index);
            start = index;
        }
    }
    ranges.push(start..count);
    Ok(())
}

#[cfg(test)]
fn group_instance_indices(keys: &[u64]) -> Result<Vec<Vec<usize>>, String> {
    let mut group_by_key = std::collections::HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        let group_index = if let Some(group_index) = group_by_key.get(key) {
            *group_index
        } else {
            groups
                .try_reserve(1)
                .map_err(|error| format!("reserving GPU instance groups failed: {error}"))?;
            group_by_key
                .try_reserve(1)
                .map_err(|error| format!("reserving GPU instance lookup failed: {error}"))?;
            let group_index = groups.len();
            groups.push(Vec::new());
            group_by_key.insert(*key, group_index);
            group_index
        };
        let group = &mut groups[group_index];
        group
            .try_reserve(1)
            .map_err(|error| format!("reserving GPU mesh instance indices failed: {error}"))?;
        group.push(index);
    }
    Ok(groups)
}

fn next_texture_descriptor_capacity(current: usize, required: usize) -> Result<usize, String> {
    let max_capacity = u32::MAX as usize;
    if required > max_capacity {
        return Err("GPU texture quad batch exceeds the descriptor limit".into());
    }
    if required <= current {
        return Ok(current);
    }

    let mut capacity = current.max(8).min(max_capacity);
    while capacity < required {
        capacity = capacity.saturating_mul(2).min(max_capacity);
    }
    Ok(capacity)
}

fn texture_descriptor_needs_update<T>(
    cached: Option<&T>,
    next: &T,
    shares_resource: impl FnOnce(&T, &T) -> bool,
) -> bool {
    !cached.is_some_and(|cached| shares_resource(cached, next))
}

fn select_present_mode(modes: &[ash::vk::PresentModeKHR]) -> ash::vk::PresentModeKHR {
    if modes.contains(&ash::vk::PresentModeKHR::MAILBOX) {
        ash::vk::PresentModeKHR::MAILBOX
    } else {
        ash::vk::PresentModeKHR::FIFO
    }
}

fn swapchain_acquire_wait_stage() -> ash::vk::PipelineStageFlags {
    ash::vk::PipelineStageFlags::TRANSFER
}

fn encode_rgb_pixels(pixels: &[[u8; 3]], bgra: bool, output: &mut Vec<u8>) -> Result<(), String> {
    let required = pixels
        .len()
        .checked_mul(4)
        .ok_or_else(|| "RGBA staging size overflow".to_owned())?;
    output.clear();
    output
        .try_reserve(required)
        .map_err(|error| format!("reserving RGBA staging pixels failed: {error}"))?;
    for &[red, green, blue] in pixels {
        if bgra {
            output.extend_from_slice(&[blue, green, red, u8::MAX]);
        } else {
            output.extend_from_slice(&[red, green, blue, u8::MAX]);
        }
    }
    Ok(())
}

fn camera_projection_runs(
    triangles: &[GpuTriangle],
) -> impl Iterator<Item = (std::ops::Range<usize>, Option<GpuCameraProjection>)> + '_ {
    let mut start = 0;
    std::iter::from_fn(move || {
        let projection = triangles.get(start)?.camera_projection;
        let run_start = start;
        start += 1;
        while start < triangles.len() && triangles[start].camera_projection == projection {
            start += 1;
        }
        Some((run_start..start, projection))
    })
}

impl GpuTriangle {
    pub fn new(points: [[f32; 2]; 3], color: [u8; 3]) -> Self {
        Self::new_rgba(points, [color[0], color[1], color[2], u8::MAX])
    }

    pub fn new_rgba(points: [[f32; 2]; 3], color: [u8; 4]) -> Self {
        Self {
            points: points.map(|[x, y]| [x, y, 0.5]),
            color,
            lighting: None,
            camera_projection: None,
            sort_depths: None,
        }
    }

    pub fn new_3d(points: [[f32; 3]; 3], color: [u8; 3]) -> Self {
        Self::new_3d_rgba(points, [color[0], color[1], color[2], u8::MAX], None)
    }

    pub fn new_3d_rgba(
        points: [[f32; 3]; 3],
        color: [u8; 4],
        lighting: Option<([f32; 3], [f32; 3], f32)>,
    ) -> Self {
        Self {
            points,
            color,
            lighting: lighting.map(|(normal, direction, ambient)| FlatLighting {
                normal,
                direction,
                ambient,
            }),
            camera_projection: None,
            sort_depths: None,
        }
    }

    pub fn new_camera_space_rgba(
        points: [[f32; 3]; 3],
        camera_projection: [f32; 4],
        sort_depths: [f32; 3],
        color: [u8; 4],
        lighting: Option<([f32; 3], [f32; 3], f32)>,
    ) -> Result<Self, String> {
        if points.into_iter().flatten().any(|value| !value.is_finite())
            || sort_depths
                .into_iter()
                .any(|depth| !depth.is_finite() || !(0.0..=1.0).contains(&depth))
        {
            return Err("camera-space GPU triangle has invalid projection data".into());
        }
        let camera_projection = GpuCameraProjection::camera_space(camera_projection)?;
        let mut triangle = Self::new_3d_rgba(points, color, lighting);
        triangle.camera_projection = Some(camera_projection);
        triangle.sort_depths = Some(sort_depths);
        Ok(triangle)
    }

    pub fn new_world_space_rgba(
        points: [[f32; 3]; 3],
        camera_projection: GpuCameraProjection,
        sort_depths: [f32; 3],
        color: [u8; 4],
        lighting: Option<([f32; 3], [f32; 3], f32)>,
    ) -> Result<Self, String> {
        if points.into_iter().flatten().any(|value| !value.is_finite())
            || sort_depths
                .into_iter()
                .any(|depth| !depth.is_finite() || !(0.0..=1.0).contains(&depth))
        {
            return Err("world-space GPU triangle has invalid camera data".into());
        }
        let mut triangle = Self::new_3d_rgba(points, color, lighting);
        triangle.camera_projection = Some(camera_projection);
        triangle.sort_depths = Some(sort_depths);
        Ok(triangle)
    }

    pub fn average_depth(&self) -> f32 {
        let depths = self
            .sort_depths
            .unwrap_or(self.points.map(|point| point[2]));
        (depths[0] + depths[1] + depths[2]) / 3.0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct GpuVertex {
    position: [f32; 3],
    color: [u8; 4],
    uv: [f32; 2],
    normal: [i8; 4],
    lighting: [i8; 4],
}

fn encode_snorm8(value: f32) -> i8 {
    (value.clamp(-1.0, 1.0) * i8::MAX as f32).round() as i8
}

fn encode_snorm4(values: [f32; 4]) -> [i8; 4] {
    values.map(encode_snorm8)
}

fn append_triangle_vertices(
    triangles: &[GpuTriangle],
    width: u32,
    height: u32,
    vertices: &mut Vec<GpuVertex>,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("GPU triangle viewport must be nonzero".to_owned());
    }
    let vertex_count = triangles
        .len()
        .checked_mul(3)
        .ok_or_else(|| "GPU triangle vertex count overflow".to_owned())?;
    u32::try_from(vertex_count)
        .map_err(|_| "GPU triangle batch exceeds Vulkan's draw vertex limit".to_owned())?;
    vertices
        .try_reserve(vertex_count)
        .map_err(|error| format!("reserving GPU triangle vertices failed: {error}"))?;
    for triangle in triangles {
        let (mut normal, lighting) =
            triangle
                .lighting
                .map_or(([0; 4], [0, 0, 0, i8::MAX]), |lighting| {
                    (
                        encode_snorm4([
                            lighting.normal[0],
                            lighting.normal[1],
                            lighting.normal[2],
                            0.0,
                        ]),
                        encode_snorm4([
                            lighting.direction[0],
                            lighting.direction[1],
                            lighting.direction[2],
                            lighting.ambient,
                        ]),
                    )
                });
        if triangle.camera_projection.is_some() {
            normal[3] = i8::MIN;
        }
        for [x, y, depth] in triangle.points {
            let position = if triangle.camera_projection.is_some() {
                if !x.is_finite() || !y.is_finite() || !depth.is_finite() {
                    return Err("GPU camera-space coordinates must be finite".into());
                }
                [x, y, depth]
            } else {
                if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
                    return Err("GPU triangle depth must be finite and between zero and one".into());
                }
                [
                    x / width as f32 * 2.0 - 1.0,
                    y / height as f32 * 2.0 - 1.0,
                    depth,
                ]
            };
            vertices.push(GpuVertex {
                position,
                color: triangle.color,
                uv: [0.0, 0.0],
                normal,
                lighting,
            });
        }
    }
    Ok(())
}

fn prepare_triangle_vertices(
    vertices: &mut Vec<GpuVertex>,
    opaque: &[GpuTriangle],
    transparent: &[GpuTriangle],
    overlay: &[GpuTriangle],
    width: u32,
    height: u32,
    trailing_vertices: usize,
) -> Result<(usize, usize, usize), String> {
    let opaque_vertices = opaque
        .len()
        .checked_mul(3)
        .ok_or_else(|| "opaque GPU triangle vertex count overflow".to_owned())?;
    let transparent_vertices = transparent
        .len()
        .checked_mul(3)
        .ok_or_else(|| "transparent GPU triangle vertex count overflow".to_owned())?;
    let overlay_vertices = overlay
        .len()
        .checked_mul(3)
        .ok_or_else(|| "GPU overlay vertex count overflow".to_owned())?;
    let triangle_vertices = opaque_vertices
        .checked_add(transparent_vertices)
        .and_then(|count| count.checked_add(overlay_vertices))
        .ok_or_else(|| "GPU triangle vertex count overflow".to_owned())?;
    let total_vertices = triangle_vertices
        .checked_add(trailing_vertices)
        .ok_or_else(|| "GPU draw vertex count overflow".to_owned())?;
    u32::try_from(total_vertices)
        .map_err(|_| "GPU draw batch exceeds Vulkan's draw vertex limit".to_owned())?;

    vertices.clear();
    vertices
        .try_reserve(total_vertices)
        .map_err(|error| format!("reserving GPU draw vertices failed: {error}"))?;
    append_triangle_vertices(opaque, width, height, vertices)?;
    append_triangle_vertices(transparent, width, height, vertices)?;
    append_triangle_vertices(overlay, width, height, vertices)?;

    Ok((opaque_vertices, transparent_vertices, overlay_vertices))
}

fn append_instanced_mesh_vertices(mesh_vertices: &[GpuMeshVertex], vertices: &mut Vec<GpuVertex>) {
    for vertex in mesh_vertices {
        vertices.push(GpuVertex {
            position: vertex.position,
            color: vertex.color,
            uv: vertex.uv,
            normal: encode_snorm4([vertex.normal[0], vertex.normal[1], vertex.normal[2], 0.0]),
            lighting: encode_snorm4([
                vertex.light_direction[0],
                vertex.light_direction[1],
                vertex.light_direction[2],
                vertex.ambient,
            ]),
        });
    }
}

fn append_camera_triangle_vertices(
    triangle: &CameraTextureTriangle,
    vertices: &mut Vec<GpuVertex>,
) {
    let lighting = triangle.lighting.map_or([0, 0, 0, i8::MAX], |lighting| {
        encode_snorm4([
            lighting.direction[0],
            lighting.direction[1],
            lighting.direction[2],
            lighting.ambient,
        ])
    });
    let normal = triangle.lighting.map_or([0, 0, 0, i8::MIN], |lighting| {
        encode_snorm4([
            lighting.normal[0],
            lighting.normal[1],
            lighting.normal[2],
            -1.0,
        ])
    });
    vertices.extend((0..3).map(|index| GpuVertex {
        position: triangle.points[index],
        color: triangle.color,
        uv: triangle.uvs[index],
        normal,
        lighting,
    }));
}

fn append_texture_quad_vertices(
    quad: &GpuTextureQuad,
    width: u32,
    height: u32,
    vertices: &mut Vec<GpuVertex>,
) -> Result<(), String> {
    if let Some(mesh) = quad.instanced_mesh.as_ref() {
        append_instanced_mesh_vertices(&mesh.vertices, vertices);
        return Ok(());
    }
    if let Some(triangle) = quad.camera_triangle.as_ref() {
        append_camera_triangle_vertices(triangle, vertices);
        return Ok(());
    }
    if width == 0 || height == 0 {
        return Err("GPU texture quad viewport must be nonzero".into());
    }
    let [x, y] = quad.position;
    let [quad_width, quad_height] = quad.size;
    let right = x + quad_width;
    let bottom = y + quad_height;
    let points = [
        ([x, y], [0.0, 0.0]),
        ([right, y], [1.0, 0.0]),
        ([right, bottom], [1.0, 1.0]),
        ([x, bottom], [0.0, 1.0]),
    ];
    let vertex = |(point, uv): ([f32; 2], [f32; 2])| GpuVertex {
        position: [
            point[0] / width as f32 * 2.0 - 1.0,
            point[1] / height as f32 * 2.0 - 1.0,
            0.5,
        ],
        color: [u8::MAX; 4],
        uv,
        normal: [0; 4],
        lighting: [0, 0, 0, i8::MAX],
    };
    vertices.extend([
        vertex(points[0]),
        vertex(points[1]),
        vertex(points[2]),
        vertex(points[0]),
        vertex(points[2]),
        vertex(points[3]),
    ]);
    Ok(())
}

fn append_unit_texture_quad_vertices(vertices: &mut Vec<GpuVertex>) -> Result<(), String> {
    vertices
        .try_reserve(6)
        .map_err(|error| format!("reserving batched GPU texture vertices failed: {error}"))?;
    let vertex = |position, uv| GpuVertex {
        position,
        color: [u8::MAX; 4],
        uv,
        normal: [0, 0, i8::MAX, 0],
        lighting: [0, 0, 0, i8::MAX],
    };
    vertices.extend([
        vertex([0.0, 0.0, 0.5], [0.0, 0.0]),
        vertex([1.0, 0.0, 0.5], [1.0, 0.0]),
        vertex([1.0, 1.0, 0.5], [1.0, 1.0]),
        vertex([0.0, 0.0, 0.5], [0.0, 0.0]),
        vertex([1.0, 1.0, 0.5], [1.0, 1.0]),
        vertex([0.0, 1.0, 0.5], [0.0, 1.0]),
    ]);
    Ok(())
}

fn append_2d_texture_instance(
    quad: &GpuTextureQuad,
    width: u32,
    height: u32,
    instances: &mut Vec<GpuInstanceData>,
) -> Result<(), String> {
    let [x, y] = quad.position;
    let [quad_width, quad_height] = quad.size;
    let width = width as f32;
    let height = height as f32;
    let scale_x = 2.0 * quad_width / width;
    let scale_y = 2.0 * quad_height / height;
    let translate_x = 2.0 * x / width - 1.0;
    let translate_y = 2.0 * y / height - 1.0;
    let values = [scale_x, scale_y, translate_x, translate_y];
    if values.iter().any(|value| !value.is_finite()) {
        return Err("GPU texture instance transform must be finite".into());
    }
    instances
        .try_reserve(1)
        .map_err(|error| format!("reserving GPU texture instances failed: {error}"))?;
    instances.push(GpuInstanceData {
        model: [
            [scale_x, 0.0, 0.0, 0.0],
            [0.0, scale_y, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [translate_x, translate_y, 0.0, 1.0],
        ],
        normal: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        enabled: [1.0, 1.0, 0.0, 0.0],
    });
    Ok(())
}

fn select_host_visible_memory_type(
    properties: &ash::vk::PhysicalDeviceMemoryProperties,
    memory_type_bits: u32,
) -> Option<(u32, bool)> {
    let find = |required| {
        (0..properties.memory_type_count).find(|index| {
            memory_type_bits & (1 << index) != 0
                && properties.memory_types[*index as usize]
                    .property_flags
                    .contains(required)
        })
    };
    find(ash::vk::MemoryPropertyFlags::HOST_VISIBLE | ash::vk::MemoryPropertyFlags::HOST_COHERENT)
        .map(|index| (index, true))
        .or_else(|| find(ash::vk::MemoryPropertyFlags::HOST_VISIBLE).map(|index| (index, false)))
}

pub(crate) fn device_selection_rank(
    device_type: ash::vk::PhysicalDeviceType,
    ray_tracing_supported: bool,
) -> (u8, u8) {
    use ash::vk::PhysicalDeviceType as DeviceType;

    let type_rank = match device_type {
        DeviceType::DISCRETE_GPU => 5,
        DeviceType::INTEGRATED_GPU => 4,
        DeviceType::VIRTUAL_GPU => 3,
        DeviceType::OTHER => 2,
        DeviceType::CPU => 1,
        _ => 0,
    };
    (type_rank, u8::from(ray_tracing_supported))
}

pub(crate) struct SharedDevice {
    pub(crate) _entry: Arc<ash::Entry>,
    pub(crate) instance: Arc<ash::Instance>,
    pub(crate) device: Arc<ash::Device>,
    pub(crate) queue: ash::vk::Queue,
    pub(crate) physical_device: ash::vk::PhysicalDevice,
    pub(crate) queue_family: u32,
    pub(crate) ray_tracing_supported: bool,
    pub(crate) queue_access: Mutex<()>,
}

impl SharedDevice {
    pub(crate) fn lock_queue(&self) -> MutexGuard<'_, ()> {
        self.queue_access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Drop for SharedDevice {
    fn drop(&mut self) {
        let _queue_guard = self.lock_queue();
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

fn instance_ref<'a>(
    shared: &'a Option<Arc<SharedDevice>>,
    instance: &'a Option<ash::Instance>,
) -> &'a ash::Instance {
    shared
        .as_ref()
        .map(|shared| shared.instance.as_ref())
        .or(instance.as_ref())
        .expect("Vulkan renderer instance is initialized")
}

fn device_ref<'a>(
    shared: &'a Option<Arc<SharedDevice>>,
    device: &'a Option<ash::Device>,
) -> &'a ash::Device {
    shared
        .as_ref()
        .map(|shared| shared.device.as_ref())
        .or(device.as_ref())
        .expect("Vulkan renderer device is initialized")
}

fn supports_ray_tracing(
    instance: &ash::Instance,
    physical_device: ash::vk::PhysicalDevice,
    extensions: &[ash::vk::ExtensionProperties],
    instance_api_version: u32,
) -> bool {
    if instance_api_version < ash::vk::API_VERSION_1_2
        || unsafe { instance.get_physical_device_properties(physical_device) }.api_version
            < ash::vk::API_VERSION_1_2
    {
        return false;
    }
    let has_extension = |name: &std::ffi::CStr| {
        extensions.iter().any(|extension| unsafe {
            std::ffi::CStr::from_ptr(extension.extension_name.as_ptr()) == name
        })
    };
    if !has_extension(ash::khr::deferred_host_operations::NAME)
        || !has_extension(ash::khr::acceleration_structure::NAME)
        || !has_extension(ash::khr::ray_tracing_pipeline::NAME)
    {
        return false;
    }

    let mut ray_tracing = ash::vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
    let mut acceleration = ash::vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
    let mut buffer_address = ash::vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
    let mut features = ash::vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut ray_tracing)
        .push_next(&mut acceleration)
        .push_next(&mut buffer_address);
    unsafe { instance.get_physical_device_features2(physical_device, &mut features) };

    ray_tracing.ray_tracing_pipeline == ash::vk::TRUE
        && acceleration.acceleration_structure == ash::vk::TRUE
        && buffer_address.buffer_device_address == ash::vk::TRUE
}

impl VulkanRenderer {
    pub fn new(window: NativeWindow, width: u32, height: u32) -> Result<Self, String> {
        let entry = unsafe { ash::Entry::load() }
            .map_err(|error| format!("loading the Vulkan loader failed: {error}"))?;
        let mut renderer = Self {
            _entry: Some(entry),
            instance: None,
            surface_loader: None,
            surface: ash::vk::SurfaceKHR::null(),
            device: None,
            shared_device: None,
            swapchain_loader: None,
            swapchain: ash::vk::SwapchainKHR::null(),
            physical_device: ash::vk::PhysicalDevice::null(),
            queue_family: 0,
            ray_tracing_supported: false,
            api_version: ash::vk::API_VERSION_1_0,
            queue: ash::vk::Queue::null(),
            command_pool: ash::vk::CommandPool::null(),
            command_buffer: ash::vk::CommandBuffer::null(),
            image_available: ash::vk::Semaphore::null(),
            render_finished: Vec::new(),
            fence: ash::vk::Fence::null(),
            staging_buffer: ash::vk::Buffer::null(),
            staging_memory: ash::vk::DeviceMemory::null(),
            staging_mapped: std::ptr::null_mut(),
            staging_size: 0,
            staging_memory_coherent: false,
            rgba_scratch: Vec::new(),
            triangle_vertex_buffer: ash::vk::Buffer::null(),
            triangle_vertex_memory: ash::vk::DeviceMemory::null(),
            triangle_vertex_mapped: std::ptr::null_mut(),
            triangle_vertex_capacity: 0,
            triangle_vertex_scratch: Vec::new(),
            texture_draw_range_scratch: Vec::new(),
            triangle_vertex_memory_coherent: false,
            last_frame_instanced_draw_count: 0,
            instance_vertex_buffer: ash::vk::Buffer::null(),
            instance_vertex_memory: ash::vk::DeviceMemory::null(),
            instance_vertex_mapped: std::ptr::null_mut(),
            instance_vertex_capacity: 0,
            instance_vertex_memory_coherent: false,
            instance_data_scratch: Vec::new(),
            texture_vertex_ranges: Vec::new(),
            texture_instance_ranges: Vec::new(),
            texture_set_layout: ash::vk::DescriptorSetLayout::null(),
            texture_descriptor_pool: ash::vk::DescriptorPool::null(),
            texture_descriptor_capacity: 0,
            texture_descriptor_sets: Vec::new(),
            texture_descriptor_textures: Vec::new(),
            default_texture_descriptor_pool: ash::vk::DescriptorPool::null(),
            default_texture_descriptor_set: ash::vk::DescriptorSet::null(),
            white_texture: None,
            in_flight_textures: Vec::new(),
            readback_supported: false,
            extent: ash::vk::Extent2D::default(),
            format: ash::vk::Format::UNDEFINED,
            swapchain_images: Vec::new(),
            swapchain_image_views: Vec::new(),
            framebuffers: Vec::new(),
            render_pass: ash::vk::RenderPass::null(),
            pipeline_layout: ash::vk::PipelineLayout::null(),
            graphics_pipeline: ash::vk::Pipeline::null(),
            transparent_pipeline: ash::vk::Pipeline::null(),
            custom_vertex_shader: None,
            custom_fragment_shader: None,
            depth_format: ash::vk::Format::UNDEFINED,
            depth_image: ash::vk::Image::null(),
            depth_memory: ash::vk::DeviceMemory::null(),
            depth_view: ash::vk::ImageView::null(),
        };
        renderer.initialize(window, width, height)?;
        Ok(renderer)
    }

    fn initialize(&mut self, window: NativeWindow, width: u32, height: u32) -> Result<(), String> {
        use ash::vk;
        let app_name = std::ffi::CString::new("Vagus Engine").unwrap();
        let loader_api_version = unsafe {
            self._entry
                .as_ref()
                .unwrap()
                .try_enumerate_instance_version()
        }
        .ok()
        .flatten()
        .unwrap_or(vk::API_VERSION_1_0);
        self.api_version = loader_api_version.min(vk::API_VERSION_1_2);
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(&app_name)
            .api_version(self.api_version);
        let mut extensions = vec![ash::khr::surface::NAME.as_ptr()];
        #[cfg(target_os = "linux")]
        match &window {
            NativeWindow::Xlib { .. } => extensions.push(ash::khr::xlib_surface::NAME.as_ptr()),
            NativeWindow::Wayland { .. } => {
                extensions.push(ash::khr::wayland_surface::NAME.as_ptr())
            }
        }
        #[cfg(target_os = "windows")]
        extensions.push(ash::khr::win32_surface::NAME.as_ptr());
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);
        self.instance = Some(
            unsafe {
                self._entry
                    .as_ref()
                    .unwrap()
                    .create_instance(&create_info, None)
            }
            .map_err(|error| format!("creating Vulkan instance failed: {error:?}"))?,
        );
        let instance = instance_ref(&self.shared_device, &self.instance);
        let entry = self._entry.as_ref().unwrap();
        self.surface_loader = Some(ash::khr::surface::Instance::new(entry, instance));
        #[cfg(target_os = "linux")]
        {
            self.surface = match window {
                NativeWindow::Xlib { display, window } => {
                    let loader = ash::khr::xlib_surface::Instance::new(entry, instance);
                    let info = vk::XlibSurfaceCreateInfoKHR::default()
                        .dpy(display.cast())
                        .window(window as vk::Window);
                    unsafe { loader.create_xlib_surface(&info, None) }.map_err(|error| {
                        format!("creating the Vulkan Xlib surface failed: {error:?}")
                    })?
                }
                NativeWindow::Wayland { display, surface } => {
                    let loader = ash::khr::wayland_surface::Instance::new(entry, instance);
                    let info = vk::WaylandSurfaceCreateInfoKHR::default()
                        .display(display.cast())
                        .surface(surface.cast());
                    unsafe { loader.create_wayland_surface(&info, None) }.map_err(|error| {
                        format!("creating the Vulkan Wayland surface failed: {error:?}")
                    })?
                }
            };
        }
        #[cfg(target_os = "windows")]
        {
            let NativeWindow::Win32 {
                instance: hinstance,
                window: hwnd,
            } = window;
            let loader = ash::khr::win32_surface::Instance::new(entry, instance);
            let info = vk::Win32SurfaceCreateInfoKHR::default()
                .hinstance(hinstance as isize)
                .hwnd(hwnd as isize);
            self.surface = unsafe { loader.create_win32_surface(&info, None) }
                .map_err(|error| format!("creating the Vulkan Win32 surface failed: {error:?}"))?;
        }
        self.select_device()?;
        self.depth_format = [
            vk::Format::D32_SFLOAT,
            vk::Format::D32_SFLOAT_S8_UINT,
            vk::Format::D24_UNORM_S8_UINT,
        ]
        .into_iter()
        .find(|format| {
            let properties = unsafe {
                instance_ref(&self.shared_device, &self.instance)
                    .get_physical_device_format_properties(self.physical_device, *format)
            };
            properties
                .optimal_tiling_features
                .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
        })
        .ok_or_else(|| "Vulkan device has no supported depth attachment format".to_owned())?;
        self.create_swapchain(width, height)?;
        self.create_graphics_pipeline()?;
        self.create_commands()?;
        self.shared_device = Some(Arc::new(SharedDevice {
            _entry: Arc::new(self._entry.take().unwrap()),
            instance: Arc::new(self.instance.take().unwrap()),
            device: Arc::new(self.device.take().unwrap()),
            queue: self.queue,
            physical_device: self.physical_device,
            queue_family: self.queue_family,
            ray_tracing_supported: self.ray_tracing_supported,
            queue_access: Mutex::new(()),
        }));
        let texture_device =
            crate::TextureDevice::from_shared(Arc::clone(self.shared_device.as_ref().unwrap()))?;
        self.white_texture = Some(texture_device.create_rgba8_texture(1, 1, &[255; 4])?);
        self.create_default_texture_descriptor()?;
        Ok(())
    }

    fn select_device(&mut self) -> Result<(), String> {
        use ash::vk;
        let instance = instance_ref(&self.shared_device, &self.instance);
        let surface_loader = self.surface_loader.as_ref().unwrap();
        let mut selected = None;
        for physical in unsafe { instance.enumerate_physical_devices() }
            .map_err(|error| format!("enumerating Vulkan devices failed: {error:?}"))?
        {
            let extensions = unsafe { instance.enumerate_device_extension_properties(physical) }
                .map_err(|error| {
                    format!("enumerating Vulkan device extensions failed: {error:?}")
                })?;
            if !extensions.iter().any(|extension| unsafe {
                std::ffi::CStr::from_ptr(extension.extension_name.as_ptr())
                    == ash::khr::swapchain::NAME
            }) {
                continue;
            }
            let ray_tracing_supported =
                supports_ray_tracing(instance, physical, &extensions, self.api_version);
            let device_type =
                unsafe { instance.get_physical_device_properties(physical) }.device_type;
            let rank = device_selection_rank(device_type, ray_tracing_supported);
            for (index, family) in
                unsafe { instance.get_physical_device_queue_family_properties(physical) }
                    .iter()
                    .enumerate()
            {
                let supports_present = unsafe {
                    surface_loader.get_physical_device_surface_support(
                        physical,
                        index as u32,
                        self.surface,
                    )
                }
                .unwrap_or(false);
                if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) && supports_present {
                    if selected.map_or(true, |(_, _, _, selected_rank)| rank > selected_rank) {
                        selected = Some((physical, index as u32, ray_tracing_supported, rank));
                    }
                }
            }
        }
        let Some((physical_device, queue_family, ray_tracing_supported, _)) = selected else {
            return Err("no Vulkan graphics device supports presentation to this window".into());
        };
        self.physical_device = physical_device;
        self.queue_family = queue_family;
        self.ray_tracing_supported = ray_tracing_supported;
        let priorities = [1.0];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(self.queue_family)
            .queue_priorities(&priorities)];
        let device = if self.ray_tracing_supported {
            let device_extensions = [
                ash::khr::swapchain::NAME.as_ptr(),
                ash::khr::deferred_host_operations::NAME.as_ptr(),
                ash::khr::acceleration_structure::NAME.as_ptr(),
                ash::khr::ray_tracing_pipeline::NAME.as_ptr(),
            ];
            let mut ray_tracing = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default()
                .ray_tracing_pipeline(true);
            let mut acceleration = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default()
                .acceleration_structure(true);
            let mut buffer_address = vk::PhysicalDeviceBufferDeviceAddressFeatures::default()
                .buffer_device_address(true);
            let device_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extensions)
                .push_next(&mut ray_tracing)
                .push_next(&mut acceleration)
                .push_next(&mut buffer_address);
            unsafe { instance.create_device(self.physical_device, &device_info, None) }
        } else {
            let device_extensions = [ash::khr::swapchain::NAME.as_ptr()];
            let device_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extensions);
            unsafe { instance.create_device(self.physical_device, &device_info, None) }
        }
        .map_err(|error| format!("creating Vulkan device failed: {error:?}"))?;
        self.device = Some(device);
        self.queue = unsafe {
            device_ref(&self.shared_device, &self.device).get_device_queue(self.queue_family, 0)
        };
        self.swapchain_loader = Some(ash::khr::swapchain::Device::new(
            instance,
            device_ref(&self.shared_device, &self.device),
        ));
        Ok(())
    }

    fn create_swapchain(&mut self, width: u32, height: u32) -> Result<(), String> {
        use ash::vk;
        let previous_format = self.format;
        let surface_loader = self.surface_loader.as_ref().unwrap();
        let caps = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
        }
        .map_err(|error| format!("querying Vulkan surface capabilities failed: {error:?}"))?;
        if !caps
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::COLOR_ATTACHMENT)
        {
            return Err(
                "Vulkan surface images must support transfer destinations and color attachments"
                    .into(),
            );
        }
        self.readback_supported = caps
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_SRC);
        let formats = unsafe {
            surface_loader.get_physical_device_surface_formats(self.physical_device, self.surface)
        }
        .map_err(|error| format!("querying Vulkan surface formats failed: {error:?}"))?;
        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(self.physical_device, self.surface)
        }
        .map_err(|error| format!("querying Vulkan surface present modes failed: {error:?}"))?;
        let present_mode = select_present_mode(&present_modes);
        self.format = formats
            .iter()
            .find(|item| {
                matches!(
                    item.format,
                    vk::Format::B8G8R8A8_UNORM
                        | vk::Format::B8G8R8A8_SRGB
                        | vk::Format::R8G8B8A8_UNORM
                        | vk::Format::R8G8B8A8_SRGB
                )
            })
            .map(|item| item.format)
            .ok_or_else(|| "Vulkan surface has no supported 8-bit RGBA/BGRA format".to_owned())?;
        let surface_format = formats
            .iter()
            .find(|item| item.format == self.format)
            .unwrap();
        self.extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: width
                    .clamp(caps.min_image_extent.width, caps.max_image_extent.width)
                    .max(1),
                height: height
                    .clamp(caps.min_image_extent.height, caps.max_image_extent.height)
                    .max(1),
            }
        };
        let count = (caps.min_image_count + 1).min(if caps.max_image_count == 0 {
            u32::MAX
        } else {
            caps.max_image_count
        });
        let composite = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::INHERIT,
        ]
        .into_iter()
        .find(|flag| caps.supported_composite_alpha.contains(*flag))
        .ok_or_else(|| "Vulkan surface has no supported composite alpha mode".to_owned())?;
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(self.extent)
            .image_array_layers(1)
            .image_usage(
                vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | if self.readback_supported {
                        vk::ImageUsageFlags::TRANSFER_SRC
                    } else {
                        vk::ImageUsageFlags::empty()
                    },
            )
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(self.swapchain);
        let new_swapchain = unsafe {
            self.swapchain_loader
                .as_ref()
                .unwrap()
                .create_swapchain(&info, None)
        }
        .map_err(|error| format!("creating Vulkan swapchain failed: {error:?}"))?;
        if self.swapchain != vk::SwapchainKHR::null() {
            self.destroy_swapchain_framebuffers();
            for semaphore in self.render_finished.drain(..) {
                unsafe {
                    device_ref(&self.shared_device, &self.device).destroy_semaphore(semaphore, None)
                };
            }
            unsafe {
                self.swapchain_loader
                    .as_ref()
                    .unwrap()
                    .destroy_swapchain(self.swapchain, None)
            };
        }
        self.swapchain = new_swapchain;
        self.swapchain_images = unsafe {
            self.swapchain_loader
                .as_ref()
                .unwrap()
                .get_swapchain_images(self.swapchain)
        }
        .map_err(|error| format!("getting Vulkan swapchain images failed: {error:?}"))?;
        self.render_finished
            .try_reserve_exact(self.swapchain_images.len())
            .map_err(|error| format!("reserving Vulkan present semaphores failed: {error}"))?;
        let device = device_ref(&self.shared_device, &self.device);
        for _ in &self.swapchain_images {
            let semaphore =
                unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                    .map_err(|error| {
                        format!("creating Vulkan present semaphore failed: {error:?}")
                    })?;
            self.render_finished.push(semaphore);
        }
        if self.render_pass != vk::RenderPass::null() {
            if previous_format != self.format {
                self.destroy_graphics_pipeline();
                self.create_graphics_pipeline()?;
            } else {
                self.create_swapchain_framebuffers()?;
            }
        }
        Ok(())
    }

    fn create_commands(&mut self) -> Result<(), String> {
        use ash::vk;
        let device = device_ref(&self.shared_device, &self.device);
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(self.queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        self.command_pool = unsafe { device.create_command_pool(&pool_info, None) }
            .map_err(|error| format!("creating Vulkan command pool failed: {error:?}"))?;
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        self.command_buffer = unsafe { device.allocate_command_buffers(&alloc_info) }
            .map_err(|error| format!("allocating Vulkan command buffer failed: {error:?}"))?[0];
        self.image_available =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .map_err(|error| format!("creating Vulkan acquire semaphore failed: {error:?}"))?;
        self.fence = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }
        .map_err(|error| format!("creating Vulkan fence failed: {error:?}"))?;
        Ok(())
    }

    fn destroy_swapchain_framebuffers(&mut self) {
        let device = device_ref(&self.shared_device, &self.device);
        unsafe {
            for framebuffer in self.framebuffers.drain(..) {
                device.destroy_framebuffer(framebuffer, None);
            }
            for view in self.swapchain_image_views.drain(..) {
                device.destroy_image_view(view, None);
            }
            if self.depth_view != ash::vk::ImageView::null() {
                device.destroy_image_view(self.depth_view, None);
                self.depth_view = ash::vk::ImageView::null();
            }
            if self.depth_image != ash::vk::Image::null() {
                device.destroy_image(self.depth_image, None);
                self.depth_image = ash::vk::Image::null();
            }
            if self.depth_memory != ash::vk::DeviceMemory::null() {
                device.free_memory(self.depth_memory, None);
                self.depth_memory = ash::vk::DeviceMemory::null();
            }
        }
    }

    fn destroy_graphics_pipeline(&mut self) {
        self.destroy_swapchain_framebuffers();
        let device = device_ref(&self.shared_device, &self.device);
        unsafe {
            if self.texture_descriptor_pool != ash::vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.texture_descriptor_pool, None);
                self.texture_descriptor_pool = ash::vk::DescriptorPool::null();
            }
            self.texture_descriptor_capacity = 0;
            self.texture_descriptor_sets.clear();
            self.texture_descriptor_textures.clear();
            if self.default_texture_descriptor_pool != ash::vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.default_texture_descriptor_pool, None);
                self.default_texture_descriptor_pool = ash::vk::DescriptorPool::null();
                self.default_texture_descriptor_set = ash::vk::DescriptorSet::null();
            }
            if self.graphics_pipeline != ash::vk::Pipeline::null() {
                device.destroy_pipeline(self.graphics_pipeline, None);
                self.graphics_pipeline = ash::vk::Pipeline::null();
            }
            if self.transparent_pipeline != ash::vk::Pipeline::null() {
                device.destroy_pipeline(self.transparent_pipeline, None);
                self.transparent_pipeline = ash::vk::Pipeline::null();
            }
            if self.pipeline_layout != ash::vk::PipelineLayout::null() {
                device.destroy_pipeline_layout(self.pipeline_layout, None);
                self.pipeline_layout = ash::vk::PipelineLayout::null();
            }
            if self.texture_set_layout != ash::vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.texture_set_layout, None);
                self.texture_set_layout = ash::vk::DescriptorSetLayout::null();
            }
            if self.render_pass != ash::vk::RenderPass::null() {
                device.destroy_render_pass(self.render_pass, None);
                self.render_pass = ash::vk::RenderPass::null();
            }
        }
    }

    fn create_swapchain_framebuffers(&mut self) -> Result<(), String> {
        use ash::vk;
        self.create_depth_attachment()?;
        let device = device_ref(&self.shared_device, &self.device);
        for &image in &self.swapchain_images {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(self.format)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1),
                );
            let view = unsafe { device.create_image_view(&view_info, None) }.map_err(|error| {
                format!("creating Vulkan swapchain image view failed: {error:?}")
            })?;
            self.swapchain_image_views.push(view);
            let attachments = [view, self.depth_view];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(self.render_pass)
                .attachments(&attachments)
                .width(self.extent.width)
                .height(self.extent.height)
                .layers(1);
            let framebuffer = unsafe { device.create_framebuffer(&framebuffer_info, None) }
                .map_err(|error| format!("creating Vulkan framebuffer failed: {error:?}"))?;
            self.framebuffers.push(framebuffer);
        }
        Ok(())
    }

    fn create_depth_attachment(&mut self) -> Result<(), String> {
        use ash::vk;
        let device = device_ref(&self.shared_device, &self.device);
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(self.depth_format)
            .extent(vk::Extent3D {
                width: self.extent.width,
                height: self.extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        self.depth_image = unsafe { device.create_image(&image_info, None) }
            .map_err(|error| format!("creating Vulkan depth image failed: {error:?}"))?;
        let requirements = unsafe { device.get_image_memory_requirements(self.depth_image) };
        let memory_properties = unsafe {
            instance_ref(&self.shared_device, &self.instance)
                .get_physical_device_memory_properties(self.physical_device)
        };
        let compatible = |index: u32| requirements.memory_type_bits & (1 << index) != 0;
        let memory_type = (0..memory_properties.memory_type_count)
            .find(|index| {
                compatible(*index)
                    && memory_properties.memory_types[*index as usize]
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .or_else(|| (0..memory_properties.memory_type_count).find(|index| compatible(*index)))
            .ok_or_else(|| "Vulkan device has no compatible depth image memory".to_owned())?;
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        self.depth_memory = unsafe { device.allocate_memory(&allocation, None) }
            .map_err(|error| format!("allocating Vulkan depth image memory failed: {error:?}"))?;
        unsafe { device.bind_image_memory(self.depth_image, self.depth_memory, 0) }
            .map_err(|error| format!("binding Vulkan depth image memory failed: {error:?}"))?;
        let view_info = vk::ImageViewCreateInfo::default()
            .image(self.depth_image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(self.depth_format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );
        self.depth_view = unsafe { device.create_image_view(&view_info, None) }
            .map_err(|error| format!("creating Vulkan depth image view failed: {error:?}"))?;
        Ok(())
    }

    fn set_fragment_shader(&mut self, words: &[u32], entry_point: &str) -> Result<(), String> {
        crate::shader::validate_spirv_words(words)?;
        std::ffi::CString::new(entry_point)
            .map_err(|_| "fragment shader entry point contains NUL".to_owned())?;
        let device = device_ref(&self.shared_device, &self.device);
        {
            let _queue_guard = self
                .shared_device
                .as_ref()
                .map(|shared| shared.lock_queue());
            unsafe { device.device_wait_idle() }.map_err(|error| {
                format!("waiting for Vulkan device before pipeline rebuild failed: {error:?}")
            })?;
        }

        let previous = self
            .custom_fragment_shader
            .replace((words.to_vec(), entry_point.to_owned()));
        self.destroy_graphics_pipeline();
        match self.create_graphics_pipeline() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.destroy_graphics_pipeline();
                self.custom_fragment_shader = previous;
                if let Err(restore_error) = self.create_graphics_pipeline() {
                    return Err(format!(
                        "creating custom fragment pipeline failed: {error}; restoring the previous pipeline also failed: {restore_error}"
                    ));
                }
                Err(error)
            }
        }
    }

    fn set_vertex_shader(&mut self, words: &[u32], entry_point: &str) -> Result<(), String> {
        crate::shader::validate_spirv_words(words)?;
        std::ffi::CString::new(entry_point)
            .map_err(|_| "vertex shader entry point contains NUL".to_owned())?;
        let device = device_ref(&self.shared_device, &self.device);
        {
            let _queue_guard = self
                .shared_device
                .as_ref()
                .map(|shared| shared.lock_queue());
            unsafe { device.device_wait_idle() }.map_err(|error| {
                format!("waiting for Vulkan device before pipeline rebuild failed: {error:?}")
            })?;
        }

        let previous = self
            .custom_vertex_shader
            .replace((words.to_vec(), entry_point.to_owned()));
        self.destroy_graphics_pipeline();
        match self.create_graphics_pipeline() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.destroy_graphics_pipeline();
                self.custom_vertex_shader = previous;
                if let Err(restore_error) = self.create_graphics_pipeline() {
                    return Err(format!(
                        "creating custom vertex pipeline failed: {error}; restoring the previous pipeline also failed: {restore_error}"
                    ));
                }
                Err(error)
            }
        }
    }

    fn create_graphics_pipeline(&mut self) -> Result<(), String> {
        use ash::vk;
        let device = device_ref(&self.shared_device, &self.device);
        let attachments = [
            vk::AttachmentDescription::default()
                .format(self.format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::LOAD)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentDescription::default()
                .format(self.depth_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        ];
        let color_attachment = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let depth_attachment = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let subpasses = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment)
            .depth_stencil_attachment(&depth_attachment)];
        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses);
        self.render_pass = unsafe { device.create_render_pass(&render_pass_info, None) }
            .map_err(|error| format!("creating Vulkan render pass failed: {error:?}"))?;

        let words = |bytes: &[u8]| {
            bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>()
        };
        let default_vertex_words =
            words(include_bytes!(concat!(env!("OUT_DIR"), "/draw.vert.spv")));
        let (vertex_words, vertex_entry_point) = self.custom_vertex_shader.as_ref().map_or(
            (default_vertex_words.as_slice(), "main"),
            |(words, entry)| (words.as_slice(), entry.as_str()),
        );
        let default_fragment_words =
            words(include_bytes!(concat!(env!("OUT_DIR"), "/draw.frag.spv")));
        let (fragment_words, fragment_entry_point) = self.custom_fragment_shader.as_ref().map_or(
            (default_fragment_words.as_slice(), "main"),
            |(words, entry)| (words.as_slice(), entry.as_str()),
        );
        let vertex_module = unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&vertex_words),
                None,
            )
        }
        .map_err(|error| format!("creating Vulkan vertex shader failed: {error:?}"))?;
        let fragment_module = match unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&fragment_words),
                None,
            )
        } {
            Ok(module) => module,
            Err(error) => {
                unsafe { device.destroy_shader_module(vertex_module, None) };
                return Err(format!("creating Vulkan fragment shader failed: {error:?}"));
            }
        };
        let vertex_entry = std::ffi::CString::new(vertex_entry_point)
            .map_err(|_| "vertex shader entry point contains NUL".to_owned())?;
        let fragment_entry = std::ffi::CString::new(fragment_entry_point)
            .map_err(|_| "fragment shader entry point contains NUL".to_owned())?;
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_module)
                .name(&vertex_entry),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_module)
                .name(&fragment_entry),
        ];
        let vertex_bindings = [
            vk::VertexInputBindingDescription::default()
                .binding(0)
                .stride(std::mem::size_of::<GpuVertex>() as u32)
                .input_rate(vk::VertexInputRate::VERTEX),
            vk::VertexInputBindingDescription::default()
                .binding(1)
                .stride(std::mem::size_of::<GpuInstanceData>() as u32)
                .input_rate(vk::VertexInputRate::INSTANCE),
        ];
        let mut vertex_attributes = vec![
            vk::VertexInputAttributeDescription::default()
                .location(0)
                .binding(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            vk::VertexInputAttributeDescription::default()
                .location(1)
                .binding(0)
                .format(vk::Format::R8G8B8A8_UNORM)
                .offset(12),
            vk::VertexInputAttributeDescription::default()
                .location(2)
                .binding(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(16),
            vk::VertexInputAttributeDescription::default()
                .location(3)
                .binding(0)
                .format(vk::Format::R8G8B8A8_SNORM)
                .offset(24),
            vk::VertexInputAttributeDescription::default()
                .location(4)
                .binding(0)
                .format(vk::Format::R8G8B8A8_SNORM)
                .offset(28),
        ];
        for (location, offset) in (5..=12).zip((0..8).map(|column| column * 16)) {
            vertex_attributes.push(
                vk::VertexInputAttributeDescription::default()
                    .location(location)
                    .binding(1)
                    .format(vk::Format::R32G32B32A32_SFLOAT)
                    .offset(offset),
            );
        }
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_bindings)
            .vertex_attribute_descriptions(&vertex_attributes);
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let color_blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let set_bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let set_layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&set_bindings);
        self.texture_set_layout =
            unsafe { device.create_descriptor_set_layout(&set_layout_info, None) }.map_err(
                |error| format!("creating Vulkan texture descriptor layout failed: {error:?}"),
            )?;
        let set_layouts = [self.texture_set_layout];
        let push_constant_ranges = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(std::mem::size_of::<GpuCameraProjection>() as u32)];
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);
        self.pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None) }
            .map_err(|error| format!("creating Vulkan pipeline layout failed: {error:?}"))?;
        let create_pipeline = |depth_write: bool| -> Result<vk::Pipeline, String> {
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(true)
                .depth_write_enable(depth_write)
                .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
            let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly)
                .viewport_state(&viewport)
                .rasterization_state(&rasterization)
                .multisample_state(&multisample)
                .depth_stencil_state(&depth_stencil)
                .color_blend_state(&color_blend)
                .dynamic_state(&dynamic)
                .layout(self.pipeline_layout)
                .render_pass(self.render_pass)
                .subpass(0)];
            match unsafe {
                device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
            } {
                Ok(mut pipelines) => Ok(pipelines.remove(0)),
                Err((pipelines, error)) => {
                    for pipeline in pipelines {
                        unsafe { device.destroy_pipeline(pipeline, None) };
                    }
                    Err(format!(
                        "creating Vulkan graphics pipeline failed: {error:?}"
                    ))
                }
            }
        };
        let graphics_result = create_pipeline(true);
        let transparent_result = if graphics_result.is_ok() {
            create_pipeline(false)
        } else {
            Err("opaque graphics pipeline creation failed".into())
        };
        unsafe {
            device.destroy_shader_module(vertex_module, None);
            device.destroy_shader_module(fragment_module, None);
        }
        self.graphics_pipeline = graphics_result?;
        self.transparent_pipeline = transparent_result?;
        if self.white_texture.is_some() {
            self.create_default_texture_descriptor()?;
        }
        self.create_swapchain_framebuffers()
    }

    fn create_default_texture_descriptor(&mut self) -> Result<(), String> {
        use ash::vk;
        let device = device_ref(&self.shared_device, &self.device);
        if self.default_texture_descriptor_pool != vk::DescriptorPool::null() {
            unsafe { device.destroy_descriptor_pool(self.default_texture_descriptor_pool, None) };
        }
        let pool_sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(1)
            .pool_sizes(&pool_sizes);
        self.default_texture_descriptor_pool = unsafe {
            device.create_descriptor_pool(&pool_info, None)
        }
        .map_err(|error| format!("creating fallback texture descriptor pool failed: {error:?}"))?;
        let layouts = [self.texture_set_layout];
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.default_texture_descriptor_pool)
            .set_layouts(&layouts);
        self.default_texture_descriptor_set = unsafe {
            device.allocate_descriptor_sets(&allocate_info)
        }
        .map_err(|error| format!("allocating fallback texture descriptor failed: {error:?}"))?[0];
        let texture = self.white_texture.as_ref().unwrap();
        let image_info = [vk::DescriptorImageInfo::default()
            .sampler(texture.sampler())
            .image_view(texture.image_view())
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let writes = [vk::WriteDescriptorSet::default()
            .dst_set(self.default_texture_descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info)];
        unsafe { device.update_descriptor_sets(&writes, &[]) };
        Ok(())
    }

    fn create_staging_buffer(&mut self) -> Result<(), String> {
        use ash::vk;
        let bytes = (self.extent.width as u64)
            .checked_mul(self.extent.height as u64)
            .and_then(|size| size.checked_mul(4))
            .ok_or_else(|| "Vulkan staging buffer size overflow".to_owned())?;
        let device = device_ref(&self.shared_device, &self.device);
        let info = vk::BufferCreateInfo::default()
            .size(bytes)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { device.create_buffer(&info, None) }
            .map_err(|error| format!("creating Vulkan staging buffer failed: {error:?}"))?;
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let memory_properties = unsafe {
            instance_ref(&self.shared_device, &self.instance)
                .get_physical_device_memory_properties(self.physical_device)
        };
        let (memory_type, coherent) = match select_host_visible_memory_type(
            &memory_properties,
            requirements.memory_type_bits,
        ) {
            Some(memory_type) => memory_type,
            None => {
                unsafe { device.destroy_buffer(buffer, None) };
                return Err("Vulkan device has no host-visible staging memory".to_owned());
            }
        };
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        let memory = match unsafe { device.allocate_memory(&alloc, None) } {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { device.destroy_buffer(buffer, None) };
                return Err(format!(
                    "allocating Vulkan staging memory failed: {error:?}"
                ));
            }
        };
        if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
            unsafe {
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
            }
            return Err(format!("binding Vulkan staging memory failed: {error:?}"));
        }
        // The frame fence guards host access to this persistently mapped buffer.
        let mapped = match unsafe {
            device.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
        } {
            Ok(mapped) => mapped,
            Err(error) => {
                unsafe {
                    device.destroy_buffer(buffer, None);
                    device.free_memory(memory, None);
                }
                return Err(format!("mapping Vulkan staging memory failed: {error:?}"));
            }
        };
        self.staging_buffer = buffer;
        self.staging_memory = memory;
        self.staging_mapped = mapped;
        self.staging_size = bytes;
        self.staging_memory_coherent = coherent;
        Ok(())
    }

    fn ensure_triangle_vertex_buffer(&mut self, required_vertices: usize) -> Result<(), String> {
        use ash::vk;
        if required_vertices <= self.triangle_vertex_capacity {
            return Ok(());
        }
        let capacity = required_vertices.max(self.triangle_vertex_capacity.saturating_mul(2));
        let bytes = u64::try_from(capacity)
            .ok()
            .and_then(|count| count.checked_mul(std::mem::size_of::<GpuVertex>() as u64))
            .ok_or_else(|| "GPU triangle vertex buffer size overflow".to_owned())?;
        let device = device_ref(&self.shared_device, &self.device);
        let info = vk::BufferCreateInfo::default()
            .size(bytes)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let new_buffer = unsafe { device.create_buffer(&info, None) }
            .map_err(|error| format!("creating GPU triangle vertex buffer failed: {error:?}"))?;
        let memory_result = (|| {
            let requirements = unsafe { device.get_buffer_memory_requirements(new_buffer) };
            let memory_properties = unsafe {
                instance_ref(&self.shared_device, &self.instance)
                    .get_physical_device_memory_properties(self.physical_device)
            };
            let (memory_type, coherent) =
                select_host_visible_memory_type(&memory_properties, requirements.memory_type_bits)
                    .ok_or_else(|| "Vulkan device has no host-visible vertex memory".to_owned())?;
            let allocation = vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type);
            let memory = unsafe { device.allocate_memory(&allocation, None) }.map_err(|error| {
                format!("allocating GPU triangle vertex memory failed: {error:?}")
            })?;
            if let Err(error) = unsafe { device.bind_buffer_memory(new_buffer, memory, 0) } {
                unsafe { device.free_memory(memory, None) };
                return Err(format!(
                    "binding GPU triangle vertex memory failed: {error:?}"
                ));
            }
            Ok((memory, coherent))
        })();
        let (new_memory, coherent) = match memory_result {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { device.destroy_buffer(new_buffer, None) };
                return Err(error);
            }
        };
        // Reuse this CPU mapping across frames; the frame fence guards each write.
        let new_mapped = match unsafe {
            device.map_memory(new_memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
        } {
            Ok(mapped) => mapped,
            Err(error) => {
                unsafe {
                    device.destroy_buffer(new_buffer, None);
                    device.free_memory(new_memory, None);
                }
                return Err(format!(
                    "mapping GPU triangle vertex memory failed: {error:?}"
                ));
            }
        };
        unsafe {
            if self.triangle_vertex_buffer != vk::Buffer::null() {
                device.destroy_buffer(self.triangle_vertex_buffer, None);
            }
            if self.triangle_vertex_memory != vk::DeviceMemory::null() {
                device.unmap_memory(self.triangle_vertex_memory);
                device.free_memory(self.triangle_vertex_memory, None);
            }
        }
        self.triangle_vertex_buffer = new_buffer;
        self.triangle_vertex_memory = new_memory;
        self.triangle_vertex_mapped = new_mapped;
        self.triangle_vertex_capacity = capacity;
        self.triangle_vertex_memory_coherent = coherent;
        Ok(())
    }

    fn ensure_instance_vertex_buffer(&mut self, required_instances: usize) -> Result<(), String> {
        use ash::vk;
        if required_instances <= self.instance_vertex_capacity {
            return Ok(());
        }
        let capacity = required_instances.max(self.instance_vertex_capacity.saturating_mul(2));
        let bytes = u64::try_from(capacity)
            .ok()
            .and_then(|count| count.checked_mul(std::mem::size_of::<GpuInstanceData>() as u64))
            .ok_or_else(|| "GPU instance buffer size overflow".to_owned())?;
        let device = device_ref(&self.shared_device, &self.device);
        let buffer_info = vk::BufferCreateInfo::default()
            .size(bytes)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { device.create_buffer(&buffer_info, None) }
            .map_err(|error| format!("creating GPU instance buffer failed: {error:?}"))?;
        let allocation = (|| {
            let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
            let properties = unsafe {
                instance_ref(&self.shared_device, &self.instance)
                    .get_physical_device_memory_properties(self.physical_device)
            };
            let (memory_type, coherent) =
                select_host_visible_memory_type(&properties, requirements.memory_type_bits)
                    .ok_or_else(|| {
                        "Vulkan device has no host-visible instance memory".to_owned()
                    })?;
            let info = vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type);
            let memory = unsafe { device.allocate_memory(&info, None) }
                .map_err(|error| format!("allocating GPU instance memory failed: {error:?}"))?;
            if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
                unsafe { device.free_memory(memory, None) };
                return Err(format!("binding GPU instance memory failed: {error:?}"));
            }
            Ok((memory, coherent))
        })();
        let (memory, coherent) = match allocation {
            Ok(value) => value,
            Err(error) => {
                unsafe { device.destroy_buffer(buffer, None) };
                return Err(error);
            }
        };
        let mapped = match unsafe {
            device.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
        } {
            Ok(mapped) => mapped,
            Err(error) => {
                unsafe {
                    device.destroy_buffer(buffer, None);
                    device.free_memory(memory, None);
                }
                return Err(format!("mapping GPU instance memory failed: {error:?}"));
            }
        };
        unsafe {
            if self.instance_vertex_buffer != vk::Buffer::null() {
                device.destroy_buffer(self.instance_vertex_buffer, None);
            }
            if self.instance_vertex_memory != vk::DeviceMemory::null() {
                device.unmap_memory(self.instance_vertex_memory);
                device.free_memory(self.instance_vertex_memory, None);
            }
        }
        self.instance_vertex_buffer = buffer;
        self.instance_vertex_memory = memory;
        self.instance_vertex_mapped = mapped;
        self.instance_vertex_capacity = capacity;
        self.instance_vertex_memory_coherent = coherent;
        Ok(())
    }

    pub fn present(
        &mut self,
        pixels: &[[u8; 3]],
        width: u32,
        height: u32,
        triangles: &[GpuTriangle],
    ) -> Result<(), String> {
        self.present_frame(
            Some(pixels),
            width,
            height,
            &[],
            &[],
            triangles,
            &[],
            [0; 3],
            false,
        )
        .map(|_| ())
    }

    pub fn present_with_textures(
        &mut self,
        pixels: &[[u8; 3]],
        width: u32,
        height: u32,
        triangles: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.present_frame(
            Some(pixels),
            width,
            height,
            &[],
            &[],
            triangles,
            texture_quads,
            [0; 3],
            false,
        )
        .map(|_| ())
    }

    pub fn present_gpu(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
    ) -> Result<(), String> {
        self.present_frame(
            None,
            width,
            height,
            &[],
            &[],
            triangles,
            &[],
            clear_color,
            false,
        )
        .map(|_| ())
    }

    pub fn present_gpu_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.present_frame(
            None,
            width,
            height,
            &[],
            &[],
            triangles,
            texture_quads,
            clear_color,
            false,
        )
        .map(|_| ())
    }

    pub fn present_gpu_readback(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
    ) -> Result<Vec<[u8; 4]>, String> {
        self.present_frame(
            None,
            width,
            height,
            &[],
            &[],
            triangles,
            &[],
            clear_color,
            true,
        )?
        .ok_or_else(|| "Vulkan frame readback returned no pixels".to_owned())
    }

    pub fn present_gpu_readback_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<Vec<[u8; 4]>, String> {
        self.present_frame(
            None,
            width,
            height,
            &[],
            &[],
            triangles,
            texture_quads,
            clear_color,
            true,
        )?
        .ok_or_else(|| "Vulkan frame readback returned no pixels".to_owned())
    }

    pub fn present_gpu_layers_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.present_frame(
            None,
            width,
            height,
            opaque,
            transparent,
            overlay,
            texture_quads,
            clear_color,
            false,
        )
        .map(|_| ())
    }

    pub fn present_gpu_readback_layers_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<Vec<[u8; 4]>, String> {
        self.present_frame(
            None,
            width,
            height,
            opaque,
            transparent,
            overlay,
            texture_quads,
            clear_color,
            true,
        )?
        .ok_or_else(|| "Vulkan frame readback returned no pixels".to_owned())
    }

    pub fn present_layers_with_textures(
        &mut self,
        pixels: &[[u8; 3]],
        width: u32,
        height: u32,
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.present_frame(
            Some(pixels),
            width,
            height,
            opaque,
            transparent,
            overlay,
            texture_quads,
            [0; 3],
            false,
        )
        .map(|_| ())
    }

    fn prepare_texture_descriptors(
        shared: &Arc<SharedDevice>,
        texture_descriptor_pool: &mut ash::vk::DescriptorPool,
        texture_descriptor_capacity: &mut usize,
        texture_descriptor_sets: &mut Vec<ash::vk::DescriptorSet>,
        texture_descriptor_textures: &mut Vec<Option<crate::GpuTexture>>,
        texture_set_layout: ash::vk::DescriptorSetLayout,
        texture_quads: &[GpuTextureQuad],
        draw_ranges: &[std::ops::Range<usize>],
    ) -> Result<(), String> {
        use ash::vk;
        if texture_quads
            .iter()
            .any(|quad| !quad.texture.is_on_shared_device(shared))
        {
            return Err("GPU texture quad uses a different Vulkan device".into());
        }
        if draw_ranges.is_empty() {
            return Ok(());
        }

        let device = shared.device.as_ref();
        let required =
            next_texture_descriptor_capacity(*texture_descriptor_capacity, draw_ranges.len())?;
        if required > *texture_descriptor_capacity {
            let descriptor_count = u32::try_from(required)
                .map_err(|_| "GPU texture quad batch exceeds the descriptor limit".to_owned())?;
            let mut layouts = Vec::new();
            layouts
                .try_reserve_exact(required)
                .map_err(|error| format!("reserving texture descriptor layouts failed: {error}"))?;
            layouts.resize(required, texture_set_layout);
            let mut new_texture_references = Vec::new();
            new_texture_references
                .try_reserve_exact(required)
                .map_err(|error| {
                    format!("reserving texture descriptor references failed: {error}")
                })?;
            new_texture_references.resize_with(required, || None);

            let pool_sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(descriptor_count)];
            let pool_info = vk::DescriptorPoolCreateInfo::default()
                .max_sets(descriptor_count)
                .pool_sizes(&pool_sizes);
            let new_pool =
                unsafe { device.create_descriptor_pool(&pool_info, None) }.map_err(|error| {
                    format!("creating Vulkan texture descriptor pool failed: {error:?}")
                })?;
            let allocate_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(new_pool)
                .set_layouts(&layouts);
            let new_sets = match unsafe { device.allocate_descriptor_sets(&allocate_info) } {
                Ok(sets) => sets,
                Err(error) => {
                    unsafe { device.destroy_descriptor_pool(new_pool, None) };
                    return Err(format!(
                        "allocating Vulkan texture descriptors failed: {error:?}"
                    ));
                }
            };

            if *texture_descriptor_pool != vk::DescriptorPool::null() {
                unsafe { device.destroy_descriptor_pool(*texture_descriptor_pool, None) };
            }
            *texture_descriptor_pool = new_pool;
            *texture_descriptor_capacity = required;
            *texture_descriptor_sets = new_sets;
            *texture_descriptor_textures = new_texture_references;
        }

        for ((range, set), cached_texture) in draw_ranges
            .iter()
            .zip(texture_descriptor_sets.iter().take(draw_ranges.len()))
            .zip(
                texture_descriptor_textures
                    .iter_mut()
                    .take(draw_ranges.len()),
            )
        {
            let texture = &texture_quads[range.start].texture;
            if !texture_descriptor_needs_update(
                cached_texture.as_ref(),
                texture,
                crate::GpuTexture::shares_resource,
            ) {
                continue;
            }
            let image_info = [vk::DescriptorImageInfo::default()
                .sampler(texture.sampler())
                .image_view(texture.image_view())
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let write = [vk::WriteDescriptorSet::default()
                .dst_set(*set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info)];
            unsafe { device.update_descriptor_sets(&write, &[]) };
            *cached_texture = Some(texture.clone());
        }
        Ok(())
    }

    fn present_frame(
        &mut self,
        pixels: Option<&[[u8; 3]]>,
        width: u32,
        height: u32,
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
        clear_color: [u8; 3],
        readback: bool,
    ) -> Result<Option<Vec<[u8; 4]>>, String> {
        use ash::vk;
        self.last_frame_instanced_draw_count = 0;
        if width == 0 || height == 0 {
            return if readback {
                Err("GPU frame readback requires nonzero dimensions".into())
            } else {
                Ok(None)
            };
        }
        if width != self.extent.width || height != self.extent.height {
            {
                let _queue_guard = self
                    .shared_device
                    .as_ref()
                    .map(|shared| shared.lock_queue());
                unsafe { device_ref(&self.shared_device, &self.device).device_wait_idle() }
                    .map_err(|error| format!("waiting before Vulkan resize failed: {error:?}"))?;
            }
            if self.staging_buffer != vk::Buffer::null() {
                if !self.staging_mapped.is_null() {
                    unsafe {
                        device_ref(&self.shared_device, &self.device)
                            .unmap_memory(self.staging_memory)
                    };
                    self.staging_mapped = std::ptr::null_mut();
                }
                unsafe {
                    device_ref(&self.shared_device, &self.device)
                        .destroy_buffer(self.staging_buffer, None)
                };
                unsafe {
                    device_ref(&self.shared_device, &self.device)
                        .free_memory(self.staging_memory, None)
                };
                self.staging_buffer = vk::Buffer::null();
                self.staging_memory = vk::DeviceMemory::null();
                self.staging_size = 0;
                self.staging_memory_coherent = false;
            }
            self.create_swapchain(width, height)?;
        }
        let expected = self.extent.width as usize * self.extent.height as usize;
        if readback && !self.readback_supported {
            return Err("Vulkan surface images do not support transfer-source readback".into());
        }
        if let Some(pixels) = pixels {
            if pixels.len() != expected {
                return Err("pixel buffer does not match Vulkan swapchain extent".into());
            }
        }
        if (pixels.is_some() || readback) && self.staging_buffer == vk::Buffer::null() {
            self.create_staging_buffer()?;
        }
        let extra_vertices = texture_quads.iter().try_fold(0usize, |total, quad| {
            total
                .checked_add(quad.instanced_mesh.as_ref().map_or_else(
                    || if quad.camera_triangle.is_some() { 3 } else { 6 },
                    |mesh| mesh.vertices.len(),
                ))
                .ok_or_else(|| "GPU texture vertex count overflow".to_owned())
        })?;
        let (opaque_vertices, transparent_vertices, _overlay_vertices) = prepare_triangle_vertices(
            &mut self.triangle_vertex_scratch,
            opaque,
            transparent,
            overlay,
            self.extent.width,
            self.extent.height,
            extra_vertices,
        )?;
        self.texture_vertex_ranges.clear();
        self.texture_instance_ranges.clear();
        self.texture_vertex_ranges
            .try_reserve(texture_quads.len())
            .map_err(|error| format!("reserving texture vertex ranges failed: {error}"))?;
        self.texture_instance_ranges
            .try_reserve(texture_quads.len())
            .map_err(|error| format!("reserving texture instance ranges failed: {error}"))?;
        self.instance_data_scratch.clear();
        self.instance_data_scratch.push(GpuInstanceData {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            normal: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
            enabled: [0.0; 4],
        });
        texture_draw_ranges(
            &mut self.texture_draw_range_scratch,
            texture_quads.len(),
            |left, right| {
                !texture_quads[left].is_instanced_mesh()
                    && !texture_quads[right].is_instanced_mesh()
                    && texture_quads[left]
                        .texture
                        .shares_resource(&texture_quads[right].texture)
                    && texture_quads[left].camera_projection()
                        == texture_quads[right].camera_projection()
                    && texture_quads[left].opaque == texture_quads[right].opaque
            },
        )?;
        for range_index in 0..self.texture_draw_range_scratch.len() {
            let range = self.texture_draw_range_scratch[range_index].clone();
            let quad_batch = &texture_quads[range.clone()];
            let instance_2d_batch = self.custom_vertex_shader.is_none()
                && range.len() >= 2
                && quad_batch
                    .iter()
                    .all(|quad| quad.camera_triangle.is_none() && quad.instanced_mesh.is_none());
            if instance_2d_batch {
                let vertex_start = self.triangle_vertex_scratch.len();
                append_unit_texture_quad_vertices(&mut self.triangle_vertex_scratch)?;
                let vertex_range = vertex_start..self.triangle_vertex_scratch.len();
                let instance_start = self.instance_data_scratch.len();
                for quad in quad_batch {
                    append_2d_texture_instance(
                        quad,
                        self.extent.width,
                        self.extent.height,
                        &mut self.instance_data_scratch,
                    )?;
                }
                let instance_range = instance_start..self.instance_data_scratch.len();
                for index in range.clone() {
                    self.texture_vertex_ranges.push(vertex_range.clone());
                    self.texture_instance_ranges
                        .push((index == range.start).then(|| instance_range.clone()));
                }
                continue;
            }

            for quad in quad_batch {
                let vertex_start = self.triangle_vertex_scratch.len();
                append_texture_quad_vertices(
                    quad,
                    self.extent.width,
                    self.extent.height,
                    &mut self.triangle_vertex_scratch,
                )?;
                self.texture_vertex_ranges
                    .push(vertex_start..self.triangle_vertex_scratch.len());
                if let Some(mesh) = quad.instanced_mesh.as_ref() {
                    let instance_start = self.instance_data_scratch.len();
                    self.instance_data_scratch
                        .extend(mesh.instances.iter().map(|instance| GpuInstanceData {
                            model: instance.model,
                            normal: instance.normal,
                            enabled: [1.0, 0.0, 0.0, 0.0],
                        }));
                    self.texture_instance_ranges
                        .push(Some(instance_start..self.instance_data_scratch.len()));
                } else {
                    self.texture_instance_ranges.push(None);
                }
            }
        }
        unsafe {
            device_ref(&self.shared_device, &self.device).wait_for_fences(
                &[self.fence],
                true,
                u64::MAX,
            )
        }
        .map_err(|error| format!("waiting for Vulkan frame fence failed: {error:?}"))?;
        self.in_flight_textures.clear();
        let shared = self
            .shared_device
            .as_ref()
            .expect("initialized renderer owns a shared device");
        Self::prepare_texture_descriptors(
            shared,
            &mut self.texture_descriptor_pool,
            &mut self.texture_descriptor_capacity,
            &mut self.texture_descriptor_sets,
            &mut self.texture_descriptor_textures,
            self.texture_set_layout,
            texture_quads,
            &self.texture_draw_range_scratch,
        )?;
        if !self.triangle_vertex_scratch.is_empty() {
            self.ensure_triangle_vertex_buffer(self.triangle_vertex_scratch.len())?;
        }
        self.ensure_instance_vertex_buffer(self.instance_data_scratch.len())?;
        let (image_index, _) = unsafe {
            self.swapchain_loader.as_ref().unwrap().acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        }
        .map_err(|error| format!("acquiring Vulkan swapchain image failed: {error:?}"))?;
        let device = device_ref(&self.shared_device, &self.device);
        if !self.triangle_vertex_scratch.is_empty() {
            let byte_len = self
                .triangle_vertex_scratch
                .len()
                .checked_mul(std::mem::size_of::<GpuVertex>())
                .ok_or_else(|| "GPU triangle vertex upload size overflow".to_owned())?;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.triangle_vertex_scratch.as_ptr().cast::<u8>(),
                    self.triangle_vertex_mapped.cast::<u8>(),
                    byte_len,
                );
            }
            let flush_result = if self.triangle_vertex_memory_coherent {
                Ok(())
            } else {
                let range = [vk::MappedMemoryRange::default()
                    .memory(self.triangle_vertex_memory)
                    .offset(0)
                    .size(vk::WHOLE_SIZE)];
                unsafe { device.flush_mapped_memory_ranges(&range) }
            };
            flush_result.map_err(|error| {
                format!("flushing GPU triangle vertex memory failed: {error:?}")
            })?;
        }
        let instance_byte_len = self
            .instance_data_scratch
            .len()
            .checked_mul(std::mem::size_of::<GpuInstanceData>())
            .ok_or_else(|| "GPU instance upload size overflow".to_owned())?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.instance_data_scratch.as_ptr().cast::<u8>(),
                self.instance_vertex_mapped.cast::<u8>(),
                instance_byte_len,
            );
        }
        if !self.instance_vertex_memory_coherent {
            let range = [vk::MappedMemoryRange::default()
                .memory(self.instance_vertex_memory)
                .offset(0)
                .size(vk::WHOLE_SIZE)];
            unsafe { device.flush_mapped_memory_ranges(&range) }
                .map_err(|error| format!("flushing GPU instance memory failed: {error:?}"))?;
        }
        if let Some(pixels) = pixels {
            let bgra = matches!(
                self.format,
                vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
            );
            encode_rgb_pixels(pixels, bgra, &mut self.rgba_scratch)?;
            if self.staging_mapped.is_null() {
                return Err("Vulkan staging memory is not mapped".to_owned());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.rgba_scratch.as_ptr(),
                    self.staging_mapped.cast::<u8>(),
                    self.rgba_scratch.len(),
                );
            }
            let flush_result = if self.staging_memory_coherent {
                Ok(())
            } else {
                let range = [vk::MappedMemoryRange::default()
                    .memory(self.staging_memory)
                    .offset(0)
                    .size(vk::WHOLE_SIZE)];
                unsafe { device.flush_mapped_memory_ranges(&range) }
            };
            flush_result
                .map_err(|error| format!("flushing Vulkan staging memory failed: {error:?}"))?;
        }
        unsafe {
            device.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
        }
        .map_err(|error| format!("resetting Vulkan command buffer failed: {error:?}"))?;
        let begin = vk::CommandBufferBeginInfo::default();
        unsafe { device.begin_command_buffer(self.command_buffer, &begin) }
            .map_err(|error| format!("beginning Vulkan command buffer failed: {error:?}"))?;
        let image = self.swapchain_images[image_index as usize];
        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);
        let to_transfer = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(range);
        unsafe {
            device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_transfer],
            );
        }
        let copy = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width: self.extent.width,
                height: self.extent.height,
                depth: 1,
            });
        if pixels.is_some() {
            unsafe {
                device.cmd_copy_buffer_to_image(
                    self.command_buffer,
                    self.staging_buffer,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[copy],
                );
            }
        } else {
            let [red, green, blue] = clear_color;
            let clear = vk::ClearColorValue {
                float32: [
                    red as f32 / 255.0,
                    green as f32 / 255.0,
                    blue as f32 / 255.0,
                    1.0,
                ],
            };
            unsafe {
                device.cmd_clear_color_image(
                    self.command_buffer,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &clear,
                    &[range],
                );
            }
        }
        let to_color = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(range);
        unsafe {
            device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_color],
            );
        }
        let clear_values = [
            vk::ClearValue::default(),
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];
        let render_begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[image_index as usize])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.extent,
            })
            .clear_values(&clear_values);
        unsafe {
            device.cmd_begin_render_pass(
                self.command_buffer,
                &render_begin,
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.graphics_pipeline,
            );
            let default_camera_projection = [0u8; std::mem::size_of::<GpuCameraProjection>()];
            device.cmd_push_constants(
                self.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                &default_camera_projection,
            );
            device.cmd_set_viewport(
                self.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.extent.width as f32,
                    height: self.extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            device.cmd_set_scissor(
                self.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.extent,
                }],
            );
            if !self.triangle_vertex_scratch.is_empty() {
                device.cmd_bind_vertex_buffers(
                    self.command_buffer,
                    0,
                    &[self.triangle_vertex_buffer, self.instance_vertex_buffer],
                    &[0, 0],
                );
                for (range, set) in self
                    .texture_draw_range_scratch
                    .iter()
                    .zip(self.texture_descriptor_sets.iter())
                {
                    let quad = &texture_quads[range.start];
                    if let Some(projection) =
                        quad.instanced_mesh.as_ref().map(|mesh| &mesh.projection)
                    {
                        device.cmd_push_constants(
                            self.command_buffer,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            projection.as_bytes(),
                        );
                    } else if let Some(parameters) = quad.camera_projection() {
                        let projection = GpuCameraProjection::camera_space(parameters)?;
                        device.cmd_push_constants(
                            self.command_buffer,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            projection.as_bytes(),
                        );
                    } else {
                        device.cmd_push_constants(
                            self.command_buffer,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            &default_camera_projection,
                        );
                    }
                    device.cmd_bind_pipeline(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        if quad.opaque {
                            self.graphics_pipeline
                        } else {
                            self.transparent_pipeline
                        },
                    );
                    device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[*set],
                        &[],
                    );
                    let vertex_start = self.texture_vertex_ranges[range.start].start;
                    let vertex_end = self.texture_vertex_ranges[range.end - 1].end;
                    let instance_range = self.texture_instance_ranges[range.start].as_ref();
                    let vertex_count = u32::try_from(vertex_end - vertex_start)
                        .map_err(|_| "GPU texture draw exceeds Vulkan vertex limits".to_owned())?;
                    let first_vertex = u32::try_from(vertex_start).map_err(|_| {
                        "GPU texture vertex offset exceeds Vulkan limits".to_owned()
                    })?;
                    let (instance_count, first_instance) = if let Some(instances) = instance_range {
                        (
                            u32::try_from(instances.len()).map_err(|_| {
                                "GPU mesh instance count exceeds Vulkan limits".to_owned()
                            })?,
                            u32::try_from(instances.start).map_err(|_| {
                                "GPU mesh instance offset exceeds Vulkan limits".to_owned()
                            })?,
                        )
                    } else {
                        (1, 0)
                    };
                    if instance_count > 1 {
                        self.last_frame_instanced_draw_count =
                            self.last_frame_instanced_draw_count.saturating_add(1);
                    }
                    device.cmd_draw(
                        self.command_buffer,
                        vertex_count,
                        instance_count,
                        first_vertex,
                        first_instance,
                    );
                }
                if !opaque.is_empty() {
                    device.cmd_bind_pipeline(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.graphics_pipeline,
                    );
                    device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[self.default_texture_descriptor_set],
                        &[],
                    );
                    for (range, projection) in camera_projection_runs(opaque) {
                        if let Some(projection) = projection {
                            device.cmd_push_constants(
                                self.command_buffer,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                projection.as_bytes(),
                            );
                        }
                        device.cmd_draw(
                            self.command_buffer,
                            ((range.end - range.start) * 3) as u32,
                            1,
                            (range.start * 3) as u32,
                            0,
                        );
                    }
                }
                if !transparent.is_empty() {
                    device.cmd_bind_pipeline(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.transparent_pipeline,
                    );
                    device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[self.default_texture_descriptor_set],
                        &[],
                    );
                    for (range, projection) in camera_projection_runs(transparent) {
                        if let Some(projection) = projection {
                            device.cmd_push_constants(
                                self.command_buffer,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                projection.as_bytes(),
                            );
                        }
                        device.cmd_draw(
                            self.command_buffer,
                            ((range.end - range.start) * 3) as u32,
                            1,
                            (opaque_vertices + range.start * 3) as u32,
                            0,
                        );
                    }
                }
                if !overlay.is_empty() {
                    device.cmd_bind_pipeline(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.graphics_pipeline,
                    );
                    device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[self.default_texture_descriptor_set],
                        &[],
                    );
                    for (range, projection) in camera_projection_runs(overlay) {
                        if let Some(projection) = projection {
                            device.cmd_push_constants(
                                self.command_buffer,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                projection.as_bytes(),
                            );
                        }
                        device.cmd_draw(
                            self.command_buffer,
                            ((range.end - range.start) * 3) as u32,
                            1,
                            (opaque_vertices + transparent_vertices + range.start * 3) as u32,
                            0,
                        );
                    }
                }
            }
            device.cmd_end_render_pass(self.command_buffer);
        }
        if readback {
            let to_transfer_src = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(range);
            unsafe {
                device.cmd_pipeline_barrier(
                    self.command_buffer,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[to_transfer_src],
                );
                device.cmd_copy_image_to_buffer(
                    self.command_buffer,
                    image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.staging_buffer,
                    &[copy],
                );
            }
        }
        let to_present = vk::ImageMemoryBarrier::default()
            .src_access_mask(if readback {
                vk::AccessFlags::TRANSFER_READ
            } else {
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            })
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(if readback {
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL
            } else {
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
            })
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(range);
        unsafe {
            device.cmd_pipeline_barrier(
                self.command_buffer,
                if readback {
                    vk::PipelineStageFlags::TRANSFER
                } else {
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                },
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_present],
            );
        }
        unsafe { device.end_command_buffer(self.command_buffer) }
            .map_err(|error| format!("ending Vulkan command buffer failed: {error:?}"))?;
        let waits = [self.image_available];
        let wait_stages = [swapchain_acquire_wait_stage()];
        let commands = [self.command_buffer];
        let render_finished = *self
            .render_finished
            .get(image_index as usize)
            .ok_or_else(|| "Vulkan swapchain image has no present semaphore".to_owned())?;
        let signals = [render_finished];
        let submit = [vk::SubmitInfo::default()
            .wait_semaphores(&waits)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&commands)
            .signal_semaphores(&signals)];
        self.in_flight_textures
            .try_reserve_exact(texture_quads.len())
            .map_err(|error| format!("retaining Vulkan frame textures failed: {error}"))?;
        unsafe { device.reset_fences(&[self.fence]) }
            .map_err(|error| format!("resetting Vulkan frame fence failed: {error:?}"))?;
        let queue_guard = self
            .shared_device
            .as_ref()
            .map(|shared| shared.lock_queue());
        unsafe { device.queue_submit(self.queue, &submit, self.fence) }
            .map_err(|error| format!("submitting Vulkan frame failed: {error:?}"))?;
        self.in_flight_textures
            .extend(texture_quads.iter().map(|quad| quad.texture.clone()));
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present = vk::PresentInfoKHR::default()
            .wait_semaphores(&signals)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe {
            self.swapchain_loader
                .as_ref()
                .unwrap()
                .queue_present(self.queue, &present)
        }
        .map_err(|error| format!("presenting Vulkan frame failed: {error:?}"))?;
        drop(queue_guard);
        if !readback {
            return Ok(None);
        }
        unsafe { device.wait_for_fences(&[self.fence], true, u64::MAX) }
            .map_err(|error| format!("waiting for Vulkan readback fence failed: {error:?}"))?;
        if self.staging_mapped.is_null() {
            return Err("Vulkan staging memory is not mapped".to_owned());
        }
        if !self.staging_memory_coherent {
            let range = [vk::MappedMemoryRange::default()
                .memory(self.staging_memory)
                .offset(0)
                .size(vk::WHOLE_SIZE)];
            if let Err(error) = unsafe { device.invalidate_mapped_memory_ranges(&range) } {
                return Err(format!(
                    "invalidating Vulkan readback memory failed: {error:?}"
                ));
            }
        }
        let bgra = matches!(
            self.format,
            vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
        );
        let bytes =
            unsafe { std::slice::from_raw_parts(self.staging_mapped.cast::<u8>(), expected * 4) };
        let pixels = bytes
            .chunks_exact(4)
            .map(|pixel| {
                if bgra {
                    [pixel[2], pixel[1], pixel[0], pixel[3]]
                } else {
                    [pixel[0], pixel[1], pixel[2], pixel[3]]
                }
            })
            .collect();
        Ok(Some(pixels))
    }
}

/// Platform-neutral window presenter used by the public rendering API.
pub struct Renderer(VulkanRenderer);

impl Renderer {
    /// Returns the number of instanced Vulkan draws recorded in the latest presented frame.
    pub fn last_frame_instanced_draw_count(&self) -> usize {
        self.0.last_frame_instanced_draw_count
    }

    pub fn new(window: &crate::WindowHandle, width: u32, height: u32) -> Result<Self, String> {
        Ok(Self(VulkanRenderer::new(
            native_window(window),
            width,
            height,
        )?))
    }

    pub fn present(
        &mut self,
        pixels: &[[u8; 3]],
        width: u32,
        height: u32,
        triangles: &[GpuTriangle],
    ) -> Result<(), String> {
        self.0.present(pixels, width, height, triangles)
    }

    pub fn present_with_textures(
        &mut self,
        pixels: &[[u8; 3]],
        width: u32,
        height: u32,
        triangles: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.0
            .present_with_textures(pixels, width, height, triangles, texture_quads)
    }

    pub fn present_gpu(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
    ) -> Result<(), String> {
        self.0.present_gpu(width, height, clear_color, triangles)
    }

    pub fn present_gpu_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.0
            .present_gpu_with_textures(width, height, clear_color, triangles, texture_quads)
    }

    pub fn present_gpu_readback(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
    ) -> Result<Vec<[u8; 4]>, String> {
        self.0
            .present_gpu_readback(width, height, clear_color, triangles)
    }

    pub fn present_gpu_readback_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        triangles: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<Vec<[u8; 4]>, String> {
        self.0.present_gpu_readback_with_textures(
            width,
            height,
            clear_color,
            triangles,
            texture_quads,
        )
    }

    pub fn present_layers_with_textures(
        &mut self,
        pixels: &[[u8; 3]],
        width: u32,
        height: u32,
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.0.present_layers_with_textures(
            pixels,
            width,
            height,
            opaque,
            transparent,
            overlay,
            texture_quads,
        )
    }

    pub fn present_gpu_layers_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<(), String> {
        self.0.present_gpu_layers_with_textures(
            width,
            height,
            clear_color,
            opaque,
            transparent,
            overlay,
            texture_quads,
        )
    }

    pub fn present_gpu_readback_layers_with_textures(
        &mut self,
        width: u32,
        height: u32,
        clear_color: [u8; 3],
        opaque: &[GpuTriangle],
        transparent: &[GpuTriangle],
        overlay: &[GpuTriangle],
        texture_quads: &[GpuTextureQuad],
    ) -> Result<Vec<[u8; 4]>, String> {
        self.0.present_gpu_readback_layers_with_textures(
            width,
            height,
            clear_color,
            opaque,
            transparent,
            overlay,
            texture_quads,
        )
    }

    /// Confirms a texture belongs to this renderer's Vulkan device.
    #[doc(hidden)]
    pub fn validate_gpu_texture(&self, texture: &crate::GpuTexture) -> Result<(), String> {
        let shared = self
            .0
            .shared_device
            .as_ref()
            .ok_or_else(|| "renderer has no shared Vulkan device".to_owned())?;
        if texture.is_on_shared_device(shared) {
            Ok(())
        } else {
            Err("GPU texture quad uses a different Vulkan device".into())
        }
    }

    pub fn shader_device(&self) -> crate::ShaderDevice {
        crate::ShaderDevice::from_shared(Arc::clone(
            self.0
                .shared_device
                .as_ref()
                .expect("initialized renderer has a shared Vulkan device"),
        ))
    }

    /// Rebuilds both graphics pipelines from fragment-stage SPIR-V and its entry point.
    pub fn set_fragment_shader(&mut self, words: &[u32], entry_point: &str) -> Result<(), String> {
        self.0.set_fragment_shader(words, entry_point)
    }

    /// Rebuilds both graphics pipelines from vertex-stage SPIR-V and its entry point.
    pub fn set_vertex_shader(&mut self, words: &[u32], entry_point: &str) -> Result<(), String> {
        self.0.set_vertex_shader(words, entry_point)
    }

    pub fn texture_device(&self) -> Result<crate::TextureDevice, String> {
        let shared = Arc::clone(
            self.0
                .shared_device
                .as_ref()
                .expect("initialized renderer has a shared Vulkan device"),
        );
        crate::TextureDevice::from_shared(shared)
    }

    pub fn ray_tracing_supported(&self) -> bool {
        self.0
            .shared_device
            .as_ref()
            .is_some_and(|shared| shared.ray_tracing_supported)
    }

    pub fn rtx_renderer(&self, width: u32, height: u32) -> Result<crate::rtx::RtxRenderer, String> {
        let shared = Arc::clone(
            self.0
                .shared_device
                .as_ref()
                .expect("initialized renderer has a shared Vulkan device"),
        );
        crate::rtx::RtxRenderer::from_shared(shared, width, height)
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            let owns_shared_device = self.shared_device.is_none();
            let device = self
                .shared_device
                .as_ref()
                .map(|shared| shared.device.as_ref())
                .or(self.device.as_ref());
            if let Some(device) = device {
                let queue_guard = self
                    .shared_device
                    .as_ref()
                    .map(|shared| shared.lock_queue());
                let _ = device.device_wait_idle();
                drop(queue_guard);
                self.in_flight_textures.clear();
                self.white_texture.take();
                if self.texture_descriptor_pool != ash::vk::DescriptorPool::null() {
                    device.destroy_descriptor_pool(self.texture_descriptor_pool, None);
                    self.texture_descriptor_pool = ash::vk::DescriptorPool::null();
                }
                if self.default_texture_descriptor_pool != ash::vk::DescriptorPool::null() {
                    device.destroy_descriptor_pool(self.default_texture_descriptor_pool, None);
                    self.default_texture_descriptor_pool = ash::vk::DescriptorPool::null();
                }
                for framebuffer in self.framebuffers.drain(..) {
                    device.destroy_framebuffer(framebuffer, None);
                }
                for view in self.swapchain_image_views.drain(..) {
                    device.destroy_image_view(view, None);
                }
                if self.depth_view != ash::vk::ImageView::null() {
                    device.destroy_image_view(self.depth_view, None);
                }
                if self.depth_image != ash::vk::Image::null() {
                    device.destroy_image(self.depth_image, None);
                }
                if self.depth_memory != ash::vk::DeviceMemory::null() {
                    device.free_memory(self.depth_memory, None);
                }
                if self.graphics_pipeline != ash::vk::Pipeline::null() {
                    device.destroy_pipeline(self.graphics_pipeline, None);
                }
                if self.transparent_pipeline != ash::vk::Pipeline::null() {
                    device.destroy_pipeline(self.transparent_pipeline, None);
                }
                if self.pipeline_layout != ash::vk::PipelineLayout::null() {
                    device.destroy_pipeline_layout(self.pipeline_layout, None);
                }
                if self.texture_set_layout != ash::vk::DescriptorSetLayout::null() {
                    device.destroy_descriptor_set_layout(self.texture_set_layout, None);
                }
                if self.render_pass != ash::vk::RenderPass::null() {
                    device.destroy_render_pass(self.render_pass, None);
                }
                if self.staging_buffer != ash::vk::Buffer::null() {
                    if !self.staging_mapped.is_null() {
                        device.unmap_memory(self.staging_memory);
                        self.staging_mapped = std::ptr::null_mut();
                    }
                    device.destroy_buffer(self.staging_buffer, None);
                }
                if self.staging_memory != ash::vk::DeviceMemory::null() {
                    device.free_memory(self.staging_memory, None);
                }
                if self.triangle_vertex_buffer != ash::vk::Buffer::null() {
                    device.destroy_buffer(self.triangle_vertex_buffer, None);
                }
                if self.triangle_vertex_memory != ash::vk::DeviceMemory::null() {
                    device.unmap_memory(self.triangle_vertex_memory);
                    device.free_memory(self.triangle_vertex_memory, None);
                }
                if self.instance_vertex_buffer != ash::vk::Buffer::null() {
                    device.destroy_buffer(self.instance_vertex_buffer, None);
                }
                if self.instance_vertex_memory != ash::vk::DeviceMemory::null() {
                    device.unmap_memory(self.instance_vertex_memory);
                    device.free_memory(self.instance_vertex_memory, None);
                }
                if self.fence != ash::vk::Fence::null() {
                    device.destroy_fence(self.fence, None);
                }
                for semaphore in self.render_finished.drain(..) {
                    device.destroy_semaphore(semaphore, None);
                }
                if self.image_available != ash::vk::Semaphore::null() {
                    device.destroy_semaphore(self.image_available, None);
                }
                if self.command_pool != ash::vk::CommandPool::null() {
                    device.destroy_command_pool(self.command_pool, None);
                }
                if self.swapchain != ash::vk::SwapchainKHR::null() {
                    if let Some(loader) = &self.swapchain_loader {
                        loader.destroy_swapchain(self.swapchain, None);
                    }
                }
                if owns_shared_device {
                    device.destroy_device(None);
                }
            }
            if self.surface != ash::vk::SurfaceKHR::null() {
                if let Some(loader) = &self.surface_loader {
                    loader.destroy_surface(self.surface, None);
                }
            }
            if owns_shared_device {
                if let Some(instance) = &self.instance {
                    instance.destroy_instance(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        append_camera_triangle_vertices, append_instanced_mesh_vertices, append_triangle_vertices,
        device_selection_rank, encode_rgb_pixels, group_instance_indices,
        next_texture_descriptor_capacity, prepare_triangle_vertices,
        select_host_visible_memory_type, select_present_mode, swapchain_acquire_wait_stage,
        texture_descriptor_needs_update, texture_draw_ranges, CameraTextureTriangle,
        GpuCameraProjection, GpuMeshVertex, GpuTriangle, GpuVertex,
    };

    #[test]
    fn staging_memory_selection_falls_back_to_noncoherent_host_visible_memory() {
        let mut properties = ash::vk::PhysicalDeviceMemoryProperties::default();
        properties.memory_type_count = 2;
        properties.memory_types[0].property_flags = ash::vk::MemoryPropertyFlags::DEVICE_LOCAL;
        properties.memory_types[1].property_flags = ash::vk::MemoryPropertyFlags::HOST_VISIBLE;

        assert_eq!(
            select_host_visible_memory_type(&properties, 0b10),
            Some((1, false))
        );
        assert_eq!(select_host_visible_memory_type(&properties, 0b01), None);
    }

    #[test]
    fn rgb_upload_scratch_reuses_capacity_and_encodes_channel_order() {
        let pixels = [[1, 2, 3], [4, 5, 6]];
        let mut scratch = Vec::with_capacity(8);
        let pointer = scratch.as_ptr();
        let capacity = scratch.capacity();

        encode_rgb_pixels(&pixels, false, &mut scratch).unwrap();
        assert_eq!(scratch, [1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(scratch.as_ptr(), pointer);
        assert_eq!(scratch.capacity(), capacity);

        encode_rgb_pixels(&pixels, true, &mut scratch).unwrap();
        assert_eq!(scratch, [3, 2, 1, 255, 6, 5, 4, 255]);
        assert_eq!(scratch.as_ptr(), pointer);
        assert_eq!(scratch.capacity(), capacity);
    }

    #[test]
    fn texture_mesh_vertex_batches_append_without_reallocating_frame_scratch() {
        let mesh_vertex = |x, color| GpuMeshVertex {
            position: [x, 2.0, 3.0],
            color: [color; 4],
            uv: [x, 0.5],
            normal: [0.0, 0.0, 1.0],
            light_direction: [1.0, 0.0, 0.0],
            ambient: 0.25,
        };
        let mut vertices = Vec::with_capacity(7);
        vertices.push(GpuVertex {
            position: [-1.0, 0.0, 0.0],
            color: [1, 2, 3, 4],
            uv: [0.0, 0.0],
            normal: [0; 4],
            lighting: [0; 4],
        });
        let pointer = vertices.as_ptr();
        let capacity = vertices.capacity();

        append_instanced_mesh_vertices(
            &[mesh_vertex(1.0, 10), mesh_vertex(2.0, 20)],
            &mut vertices,
        );
        append_instanced_mesh_vertices(
            &[
                mesh_vertex(3.0, 30),
                mesh_vertex(4.0, 40),
                mesh_vertex(5.0, 50),
            ],
            &mut vertices,
        );

        assert_eq!(
            vertices
                .iter()
                .map(|vertex| vertex.position[0])
                .collect::<Vec<_>>(),
            [-1.0, 1.0, 2.0, 3.0, 4.0, 5.0]
        );
        assert_eq!(vertices[0].color, [1, 2, 3, 4]);
        assert_eq!(vertices[1].color, [10; 4]);
        assert_eq!(vertices[5].color, [50; 4]);
        assert_eq!(vertices.as_ptr(), pointer);
        assert_eq!(vertices.capacity(), capacity);
    }

    #[test]
    fn camera_textured_triangle_stages_only_its_three_vertices() {
        let triangle = CameraTextureTriangle {
            points: [[-0.5, 0.25, 2.0], [0.5, 0.25, 2.0], [0.0, -0.5, 2.0]],
            uvs: [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
            projection: [1.0, 1.0, 0.1, 100.0],
            sort_depth: 2.0,
            color: [10, 20, 30, 40],
            lighting: None,
        };
        let mut vertices = Vec::with_capacity(4);
        let pointer = vertices.as_ptr();
        let capacity = vertices.capacity();

        append_camera_triangle_vertices(&triangle, &mut vertices);

        assert_eq!(vertices.len(), 3);
        assert_eq!(
            vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>(),
            triangle.points
        );
        assert_eq!(
            vertices.iter().map(|vertex| vertex.uv).collect::<Vec<_>>(),
            triangle.uvs
        );
        assert!(vertices.iter().all(|vertex| vertex.color == triangle.color));
        assert_eq!(vertices.as_ptr(), pointer);
        assert_eq!(vertices.capacity(), capacity);
    }

    #[test]
    fn vulkan_device_selection_prefers_discrete_gpus_then_ray_tracing() {
        use ash::vk::PhysicalDeviceType as DeviceType;

        assert!(
            device_selection_rank(DeviceType::DISCRETE_GPU, false)
                > device_selection_rank(DeviceType::INTEGRATED_GPU, false)
        );
        assert!(
            device_selection_rank(DeviceType::DISCRETE_GPU, true)
                > device_selection_rank(DeviceType::DISCRETE_GPU, false)
        );
        assert!(
            device_selection_rank(DeviceType::DISCRETE_GPU, false)
                > device_selection_rank(DeviceType::CPU, true)
        );
    }

    #[test]
    fn swapchain_prefers_mailbox_and_falls_back_to_fifo() {
        use ash::vk::PresentModeKHR as Mode;

        assert_eq!(select_present_mode(&[Mode::FIFO]), Mode::FIFO);
        assert_eq!(
            select_present_mode(&[Mode::FIFO, Mode::MAILBOX]),
            Mode::MAILBOX
        );
        assert_eq!(
            select_present_mode(&[Mode::IMMEDIATE, Mode::FIFO]),
            Mode::FIFO
        );
    }

    #[test]
    fn swapchain_acquire_waits_before_transfer_image_access() {
        assert_eq!(
            swapchain_acquire_wait_stage(),
            ash::vk::PipelineStageFlags::TRANSFER
        );
    }

    #[test]
    fn texture_descriptor_cache_detects_reused_and_changed_resources() {
        let cached = 7;
        let same_resource = |left: &u8, right: &u8| left == right;

        assert!(texture_descriptor_needs_update(
            None,
            &cached,
            same_resource
        ));
        assert!(!texture_descriptor_needs_update(
            Some(&cached),
            &cached,
            same_resource
        ));
        assert!(texture_descriptor_needs_update(
            Some(&cached),
            &8,
            same_resource
        ));
    }

    #[test]
    fn texture_descriptor_capacity_grows_geometrically_and_rejects_overflow() {
        assert_eq!(next_texture_descriptor_capacity(0, 0).unwrap(), 0);
        assert_eq!(next_texture_descriptor_capacity(0, 1).unwrap(), 8);
        assert_eq!(next_texture_descriptor_capacity(8, 8).unwrap(), 8);
        assert_eq!(next_texture_descriptor_capacity(8, 9).unwrap(), 16);
        assert_eq!(next_texture_descriptor_capacity(16, 17).unwrap(), 32);
        assert!(next_texture_descriptor_capacity(0, usize::MAX).is_err());
    }

    #[test]
    fn texture_draw_ranges_reuse_capacity_and_replace_old_ranges() {
        let texture_keys = [1, 1, 2, 1, 1];
        let mut ranges = Vec::with_capacity(texture_keys.len());
        let pointer = ranges.as_ptr();
        let capacity = ranges.capacity();
        texture_draw_ranges(&mut ranges, texture_keys.len(), |left, right| {
            texture_keys[left] == texture_keys[right]
        })
        .unwrap();

        assert_eq!(ranges, [0..2, 2..3, 3..5]);
        let next_texture_keys = [2, 2];
        texture_draw_ranges(&mut ranges, next_texture_keys.len(), |left, right| {
            next_texture_keys[left] == next_texture_keys[right]
        })
        .unwrap();

        assert_eq!(ranges, [0..2]);
        assert_eq!(ranges.as_ptr(), pointer);
        assert_eq!(ranges.capacity(), capacity);
        texture_draw_ranges(&mut ranges, 0, |_, _| unreachable!()).unwrap();
        assert!(ranges.is_empty());
        assert_eq!(ranges.as_ptr(), pointer);
        assert_eq!(ranges.capacity(), capacity);
    }

    #[test]
    fn mesh_instances_group_by_key_preserving_first_seen_group_order() {
        let groups = group_instance_indices(&[7, 4, 7, 9, 4, 7]).unwrap();

        assert_eq!(groups, [vec![0, 2, 5], vec![1, 4], vec![3]]);
    }

    #[test]
    fn vertex_memory_prefers_coherent_and_accepts_noncoherent_host_memory() {
        let mut properties = ash::vk::PhysicalDeviceMemoryProperties::default();
        properties.memory_type_count = 2;
        properties.memory_types[0].property_flags = ash::vk::MemoryPropertyFlags::HOST_VISIBLE;
        properties.memory_types[1].property_flags = ash::vk::MemoryPropertyFlags::HOST_VISIBLE
            | ash::vk::MemoryPropertyFlags::HOST_COHERENT;

        assert_eq!(
            select_host_visible_memory_type(&properties, 0b11),
            Some((1, true))
        );
        assert_eq!(
            select_host_visible_memory_type(&properties, 0b01),
            Some((0, false))
        );
        assert_eq!(select_host_visible_memory_type(&properties, 0), None);
    }

    #[test]
    fn gpu_triangle_batch_encodes_vertex_positions_and_colors() {
        let triangles = [
            GpuTriangle::new([[0.0, 0.0], [32.0, 0.0], [0.0, 32.0]], [255, 0, 0]),
            GpuTriangle::new([[64.0, 64.0], [32.0, 64.0], [64.0, 32.0]], [0, 255, 0]),
        ];
        let mut vertices = Vec::with_capacity(6);
        append_triangle_vertices(&triangles, 64, 64, &mut vertices).unwrap();

        assert_eq!(vertices.len(), 6);
        assert_eq!(
            vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>(),
            [
                [-1.0, -1.0, 0.5],
                [0.0, -1.0, 0.5],
                [-1.0, 0.0, 0.5],
                [1.0, 1.0, 0.5],
                [0.0, 1.0, 0.5],
                [1.0, 0.0, 0.5],
            ]
        );
        assert_eq!(
            vertices
                .iter()
                .map(|vertex| vertex.color)
                .collect::<Vec<_>>(),
            [
                [255, 0, 0, 255],
                [255, 0, 0, 255],
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 255, 0, 255],
                [0, 255, 0, 255],
            ]
        );
        assert_eq!(std::mem::size_of::<super::GpuVertex>(), 32);
    }

    #[test]
    fn gpu_vertex_lighting_uses_clamped_snorm8_channels() {
        let triangle = GpuTriangle::new_3d_rgba(
            [[0.0, 0.0, 0.5]; 3],
            [255; 4],
            Some(([1.0, -1.0, 0.5], [-2.0, 0.0, 1.0], 0.25)),
        );
        let mut vertices = Vec::new();
        append_triangle_vertices(&[triangle], 64, 64, &mut vertices).unwrap();

        assert_eq!(vertices[0].normal, [127, -127, 64, 0]);
        assert_eq!(vertices[0].lighting, [-127, 0, 127, 32]);
        assert_eq!(std::mem::size_of::<super::GpuVertex>(), 32);
    }

    #[test]
    fn camera_space_vertices_preserve_view_coordinates_and_projection_parameters() {
        let triangle = GpuTriangle::new_camera_space_rgba(
            [[-0.5, -0.25, 2.0], [0.5, -0.25, 2.0], [0.0, 0.5, 2.0]],
            [1.5, 1.0, 0.1, 100.0],
            [0.95, 0.95, 0.95],
            [255, 255, 255, 255],
            None,
        )
        .unwrap();
        let mut vertices = Vec::new();
        append_triangle_vertices(&[triangle], 64, 64, &mut vertices).unwrap();

        assert_eq!(vertices[0].position[0], -0.5);
        assert_eq!(vertices[0].position[1], -0.25);
        assert_eq!(vertices[0].position[2], 2.0);
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[0].normal[3], i8::MIN);
        assert!((triangle.average_depth() - 0.95).abs() < 0.0001);
        assert!(
            GpuTriangle::new_camera_space_rgba(
                [[0.0, 0.0, 2.0]; 3],
                [1.0, 1.0, 0.1, 100.0],
                [0.5, 0.5, 1.1],
                [255; 4],
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn camera_space_projection_disables_shader_view_transform() {
        let camera_space = GpuCameraProjection::camera_space([1.5, 1.0, 0.1, 100.0]).unwrap();
        let world_space = GpuCameraProjection::new(
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.5, 1.0, 0.1, 100.0],
        )
        .unwrap();

        assert_eq!(camera_space.origin[3], 0.0);
        assert_eq!(world_space.origin[3], 1.0);
    }

    #[test]
    fn world_space_vertices_preserve_unprojected_coordinates_and_camera_data() {
        let camera = GpuCameraProjection::new(
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.5, 1.0, 0.1, 100.0],
        )
        .unwrap();
        let points = [[2.0, 3.0, 5.0], [3.0, 3.0, 5.0], [2.5, 4.0, 5.0]];
        let triangle =
            GpuTriangle::new_world_space_rgba(points, camera, [0.5; 3], [255; 4], None).unwrap();
        let mut vertices = Vec::new();

        append_triangle_vertices(&[triangle], 64, 64, &mut vertices).unwrap();

        assert_eq!(
            vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>(),
            points
        );
        assert!(vertices.iter().all(|vertex| vertex.uv == [0.0, 0.0]));
        assert!(vertices.iter().all(|vertex| vertex.normal[3] == i8::MIN));
        assert_eq!(triangle.camera_projection, Some(camera));
    }

    #[test]
    fn camera_projection_parameters_are_not_repeated_per_vertex() {
        assert_eq!(std::mem::size_of::<super::GpuVertex>(), 32);
        assert_eq!(std::mem::size_of::<GpuCameraProjection>(), 80);
    }

    #[test]
    fn camera_projection_runs_group_adjacent_equal_projection_data() {
        let projection = [1.5, 1.0, 0.1, 100.0];
        let camera_projection = GpuCameraProjection::camera_space(projection).unwrap();
        let triangles = [
            GpuTriangle::new_camera_space_rgba(
                [[0.0, 0.0, 2.0]; 3],
                projection,
                [0.5; 3],
                [255; 4],
                None,
            )
            .unwrap(),
            GpuTriangle::new_camera_space_rgba(
                [[0.0, 0.0, 3.0]; 3],
                projection,
                [0.75; 3],
                [255; 4],
                None,
            )
            .unwrap(),
            GpuTriangle::new_camera_space_rgba(
                [[0.0, 0.0, 2.0]; 3],
                [2.0, 1.0, 0.1, 100.0],
                [0.5; 3],
                [255; 4],
                None,
            )
            .unwrap(),
            GpuTriangle::new([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], [255; 3]),
        ];

        let runs = super::camera_projection_runs(&triangles).collect::<Vec<_>>();

        assert_eq!(
            runs,
            [
                (0..2, Some(camera_projection)),
                (
                    2..3,
                    Some(GpuCameraProjection::camera_space([2.0, 1.0, 0.1, 100.0]).unwrap())
                ),
                (3..4, None),
            ]
        );
    }

    #[test]
    fn triangle_vertex_layers_append_into_preallocated_storage() {
        let triangles = [
            GpuTriangle::new([[0.0, 0.0], [32.0, 0.0], [0.0, 32.0]], [255, 0, 0]),
            GpuTriangle::new([[64.0, 64.0], [32.0, 64.0], [64.0, 32.0]], [0, 255, 0]),
        ];
        let mut vertices = Vec::with_capacity(6);
        let storage = vertices.as_ptr();
        let capacity = vertices.capacity();

        append_triangle_vertices(&triangles[..1], 64, 64, &mut vertices).unwrap();
        append_triangle_vertices(&triangles[1..], 64, 64, &mut vertices).unwrap();

        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices.as_ptr(), storage);
        assert_eq!(vertices.capacity(), capacity);
        assert_eq!(vertices[0].color, [255, 0, 0, 255]);
        assert_eq!(vertices[3].color, [0, 255, 0, 255]);
    }

    #[test]
    fn triangle_frame_staging_reuses_capacity_and_replaces_previous_frame() {
        let opaque = [GpuTriangle::new(
            [[0.0, 0.0], [8.0, 0.0], [0.0, 8.0]],
            [255, 0, 0],
        )];
        let transparent = [GpuTriangle::new_rgba(
            [[8.0, 8.0], [0.0, 8.0], [8.0, 0.0]],
            [0, 255, 0, 128],
        )];
        let overlay = [GpuTriangle::new(
            [[1.0, 1.0], [2.0, 1.0], [1.0, 2.0]],
            [0, 0, 255],
        )];
        let mut vertices = Vec::new();

        prepare_triangle_vertices(&mut vertices, &opaque, &transparent, &overlay, 8, 8, 0).unwrap();
        let storage = vertices.as_ptr();
        let capacity = vertices.capacity();
        assert_eq!(vertices.len(), 9);
        assert_eq!(vertices[0].color, [255, 0, 0, 255]);
        assert_eq!(vertices[3].color, [0, 255, 0, 128]);
        assert_eq!(vertices[6].color, [0, 0, 255, 255]);

        prepare_triangle_vertices(&mut vertices, &opaque, &[], &[], 8, 8, 0).unwrap();

        assert_eq!(vertices.as_ptr(), storage);
        assert_eq!(vertices.capacity(), capacity);
        assert_eq!(vertices.len(), 3);
        assert_eq!(vertices[0].color, [255, 0, 0, 255]);
    }
}
