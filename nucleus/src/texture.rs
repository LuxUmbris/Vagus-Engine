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

use crate::vulkan::SharedDevice;
use ash::{Device, Entry, Instance, vk};
use std::{
    ffi::CString,
    ptr,
    sync::{Arc, Mutex, MutexGuard},
};

pub fn validate_rgba8_texture_data(width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("texture dimensions must be nonzero".into());
    }
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()?
                .checked_mul(width)?
                .checked_mul(4)
        })
        .ok_or_else(|| "texture dimensions overflow address space".to_owned())?;
    if pixels.len() != expected {
        return Err(format!(
            "RGBA8 texture payload must contain {expected} bytes, got {}",
            pixels.len()
        ));
    }
    Ok(())
}

enum DeviceOwner {
    Owned {
        _entry: Entry,
        instance: Instance,
        physical_device: vk::PhysicalDevice,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
    },
    Shared {
        shared: Arc<SharedDevice>,
        command_pool: vk::CommandPool,
    },
}

struct DeviceInner {
    owner: DeviceOwner,
    owned_queue_access: Mutex<()>,
}

impl DeviceInner {
    fn lock_queue(&self) -> MutexGuard<'_, ()> {
        let queue_access = match &self.owner {
            DeviceOwner::Owned { .. } => &self.owned_queue_access,
            DeviceOwner::Shared { shared, .. } => &shared.queue_access,
        };
        queue_access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn instance(&self) -> &Instance {
        match &self.owner {
            DeviceOwner::Owned { instance, .. } => instance,
            DeviceOwner::Shared { shared, .. } => &shared.instance,
        }
    }

    fn physical_device(&self) -> vk::PhysicalDevice {
        match &self.owner {
            DeviceOwner::Owned {
                physical_device, ..
            } => *physical_device,
            DeviceOwner::Shared { shared, .. } => shared.physical_device,
        }
    }

    fn device(&self) -> &Device {
        match &self.owner {
            DeviceOwner::Owned { device, .. } => device,
            DeviceOwner::Shared { shared, .. } => &shared.device,
        }
    }

    fn queue(&self) -> vk::Queue {
        match &self.owner {
            DeviceOwner::Owned { queue, .. } => *queue,
            DeviceOwner::Shared { shared, .. } => shared.queue,
        }
    }

    fn command_pool(&self) -> vk::CommandPool {
        match &self.owner {
            DeviceOwner::Owned { command_pool, .. } | DeviceOwner::Shared { command_pool, .. } => {
                *command_pool
            }
        }
    }
}

impl Drop for DeviceInner {
    fn drop(&mut self) {
        let _queue_guard = self.lock_queue();
        unsafe {
            match &self.owner {
                DeviceOwner::Owned {
                    instance,
                    device,
                    command_pool,
                    ..
                } => {
                    let _ = device.device_wait_idle();
                    device.destroy_command_pool(*command_pool, None);
                    device.destroy_device(None);
                    instance.destroy_instance(None);
                }
                DeviceOwner::Shared {
                    shared,
                    command_pool,
                } => shared.device.destroy_command_pool(*command_pool, None),
            }
        }
    }
}

#[derive(Clone)]
pub struct TextureDevice {
    inner: Arc<DeviceInner>,
}

#[derive(Clone)]
pub struct GpuTexture {
    resource: Arc<GpuTextureResource>,
}

struct GpuTextureResource {
    inner: Arc<DeviceInner>,
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    sampler: vk::Sampler,
    width: u32,
    height: u32,
}

impl TextureDevice {
    /// Creates a standalone Vulkan device for texture uploads.
    /// Use `Renderer::texture_device` to share the presentation device.
    pub fn new_standalone() -> Result<Self, String> {
        let entry =
            unsafe { Entry::load() }.map_err(|error| format!("failed to load Vulkan: {error}"))?;
        let app_name = CString::new("Vagus texture device").expect("static string has no NUL");
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(&app_name)
            .api_version(vk::API_VERSION_1_0);
        let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);
        let instance = unsafe { entry.create_instance(&instance_info, None) }
            .map_err(|error| format!("failed to create Vulkan instance: {error}"))?;

        let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
            Ok(devices) => devices,
            Err(error) => {
                unsafe { instance.destroy_instance(None) };
                return Err(format!("failed to enumerate Vulkan devices: {error}"));
            }
        };
        let selected = physical_devices.into_iter().find_map(|physical| {
            unsafe { instance.get_physical_device_queue_family_properties(physical) }
                .iter()
                .position(|family| {
                    family.queue_count > 0 && family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                })
                .map(|index| (physical, index as u32))
        });
        let Some((physical_device, queue_family)) = selected else {
            unsafe { instance.destroy_instance(None) };
            return Err("no Vulkan graphics queue is available for texture uploads".into());
        };

        let priorities = [1.0_f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_info);
        let device = match unsafe { instance.create_device(physical_device, &device_info, None) } {
            Ok(device) => device,
            Err(error) => {
                unsafe { instance.destroy_instance(None) };
                return Err(format!("failed to create Vulkan texture device: {error}"));
            }
        };
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT);
        let command_pool = match unsafe { device.create_command_pool(&pool_info, None) } {
            Ok(pool) => pool,
            Err(error) => {
                unsafe {
                    device.destroy_device(None);
                    instance.destroy_instance(None);
                }
                return Err(format!(
                    "failed to create Vulkan texture command pool: {error}"
                ));
            }
        };

        Ok(Self {
            inner: Arc::new(DeviceInner {
                owner: DeviceOwner::Owned {
                    _entry: entry,
                    instance,
                    physical_device,
                    device,
                    queue,
                    command_pool,
                },
                owned_queue_access: Mutex::new(()),
            }),
        })
    }

    pub(crate) fn from_shared(shared: Arc<SharedDevice>) -> Result<Self, String> {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(shared.queue_family)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT);
        let command_pool =
            unsafe { shared.device.create_command_pool(&pool_info, None) }.map_err(|error| {
                format!("failed to create shared Vulkan texture command pool: {error}")
            })?;
        Ok(Self {
            inner: Arc::new(DeviceInner {
                owner: DeviceOwner::Shared {
                    shared,
                    command_pool,
                },
                owned_queue_access: Mutex::new(()),
            }),
        })
    }

    pub fn create_rgba8_texture(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<GpuTexture, String> {
        validate_rgba8_texture_data(width, height, pixels)?;
        let byte_len = pixels.len() as vk::DeviceSize;
        let staging = StagingBuffer::new(Arc::clone(&self.inner), byte_len)?;
        staging.write(pixels)?;

        let device = self.inner.device();
        let extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(extent)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = unsafe { device.create_image(&image_info, None) }
            .map_err(|error| format!("failed to create Vulkan texture image: {error}"))?;
        let mut texture = GpuTextureResource {
            inner: Arc::clone(&self.inner),
            image,
            memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            sampler: vk::Sampler::null(),
            width,
            height,
        };

        let requirements = unsafe { device.get_image_memory_requirements(texture.image) };
        let memory_type = find_memory_type(
            &self.inner,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::empty(),
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        texture.memory = unsafe { device.allocate_memory(&allocation, None) }
            .map_err(|error| format!("failed to allocate Vulkan texture memory: {error}"))?;
        unsafe { device.bind_image_memory(texture.image, texture.memory, 0) }
            .map_err(|error| format!("failed to bind Vulkan texture memory: {error}"))?;

        let view_info = vk::ImageViewCreateInfo::default()
            .image(texture.image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .subresource_range(color_subresource_range());
        texture.view = unsafe { device.create_image_view(&view_info, None) }
            .map_err(|error| format!("failed to create Vulkan texture view: {error}"))?;
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .max_lod(0.0);
        texture.sampler = unsafe { device.create_sampler(&sampler_info, None) }
            .map_err(|error| format!("failed to create Vulkan texture sampler: {error}"))?;

        upload_image(&self.inner, &staging, &texture)?;
        Ok(GpuTexture {
            resource: Arc::new(texture),
        })
    }
}

impl GpuTexture {
    pub fn width(&self) -> u32 {
        self.resource.width
    }

    pub fn height(&self) -> u32 {
        self.resource.height
    }

    pub(crate) fn image_view(&self) -> vk::ImageView {
        self.resource.view
    }

    pub(crate) fn sampler(&self) -> vk::Sampler {
        self.resource.sampler
    }

    pub(crate) fn shares_resource(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.resource, &other.resource)
    }

    pub(crate) fn is_on_shared_device(&self, shared: &Arc<SharedDevice>) -> bool {
        matches!(
            &self.resource.inner.owner,
            DeviceOwner::Shared { shared: owner, .. } if Arc::ptr_eq(owner, shared)
        )
    }
}

impl Drop for GpuTextureResource {
    fn drop(&mut self) {
        unsafe {
            let device = self.inner.device();
            if self.sampler != vk::Sampler::null() {
                device.destroy_sampler(self.sampler, None);
            }
            if self.view != vk::ImageView::null() {
                device.destroy_image_view(self.view, None);
            }
            if self.image != vk::Image::null() {
                device.destroy_image(self.image, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                device.free_memory(self.memory, None);
            }
        }
    }
}

struct StagingBuffer {
    inner: Arc<DeviceInner>,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    host_coherent: bool,
}

impl StagingBuffer {
    fn new(inner: Arc<DeviceInner>, size: vk::DeviceSize) -> Result<Self, String> {
        let device = inner.device();
        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { device.create_buffer(&info, None) }
            .map_err(|error| format!("failed to create Vulkan texture staging buffer: {error}"))?;
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let memory_type = match find_memory_type(
            &inner,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
            vk::MemoryPropertyFlags::HOST_COHERENT,
        ) {
            Ok(memory_type) => memory_type,
            Err(error) => {
                unsafe { device.destroy_buffer(buffer, None) };
                return Err(error);
            }
        };
        let properties = unsafe {
            inner
                .instance()
                .get_physical_device_memory_properties(inner.physical_device())
        };
        let host_coherent = properties.memory_types[memory_type as usize]
            .property_flags
            .contains(vk::MemoryPropertyFlags::HOST_COHERENT);
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        let memory = match unsafe { device.allocate_memory(&allocation, None) } {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { device.destroy_buffer(buffer, None) };
                return Err(format!("failed to allocate Vulkan staging memory: {error}"));
            }
        };
        if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
            unsafe {
                device.free_memory(memory, None);
                device.destroy_buffer(buffer, None);
            }
            return Err(format!("failed to bind Vulkan staging memory: {error}"));
        }
        Ok(Self {
            inner,
            buffer,
            memory,
            size,
            host_coherent,
        })
    }

    fn write(&self, pixels: &[u8]) -> Result<(), String> {
        let device = self.inner.device();
        let mapped =
            unsafe { device.map_memory(self.memory, 0, self.size, vk::MemoryMapFlags::empty()) }
                .map_err(|error| format!("failed to map Vulkan staging memory: {error}"))?;
        unsafe {
            ptr::copy_nonoverlapping(pixels.as_ptr(), mapped.cast::<u8>(), pixels.len());
        }
        let flush_result = if self.host_coherent {
            Ok(())
        } else {
            let range = [vk::MappedMemoryRange::default()
                .memory(self.memory)
                .offset(0)
                .size(vk::WHOLE_SIZE)];
            unsafe { device.flush_mapped_memory_ranges(&range) }
                .map_err(|error| format!("failed to flush Vulkan staging memory: {error}"))
        };
        unsafe { device.unmap_memory(self.memory) };
        flush_result
    }
}

impl Drop for StagingBuffer {
    fn drop(&mut self) {
        unsafe {
            self.inner.device().destroy_buffer(self.buffer, None);
            self.inner.device().free_memory(self.memory, None);
        }
    }
}

struct CommandBufferGuard {
    inner: Arc<DeviceInner>,
    command_buffer: vk::CommandBuffer,
}

impl Drop for CommandBufferGuard {
    fn drop(&mut self) {
        unsafe {
            self.inner
                .device()
                .free_command_buffers(self.inner.command_pool(), &[self.command_buffer]);
        }
    }
}

fn upload_image(
    inner: &Arc<DeviceInner>,
    staging: &StagingBuffer,
    texture: &GpuTextureResource,
) -> Result<(), String> {
    let device = inner.device();
    let allocate = vk::CommandBufferAllocateInfo::default()
        .command_pool(inner.command_pool())
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let command_buffer = unsafe { device.allocate_command_buffers(&allocate) }
        .map_err(|error| format!("failed to allocate texture upload command buffer: {error}"))?[0];
    let command = CommandBufferGuard {
        inner: Arc::clone(inner),
        command_buffer,
    };
    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe { device.begin_command_buffer(command.command_buffer, &begin) }
        .map_err(|error| format!("failed to begin texture upload command buffer: {error}"))?;

    let range = color_subresource_range();
    let to_transfer = vk::ImageMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(texture.image)
        .subresource_range(range);
    let host_writes = [vk::BufferMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::HOST_WRITE)
        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .buffer(staging.buffer)
        .offset(0)
        .size(staging.size)];
    unsafe {
        device.cmd_pipeline_barrier(
            command.command_buffer,
            vk::PipelineStageFlags::HOST,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &host_writes,
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
            width: texture.width,
            height: texture.height,
            depth: 1,
        });
    unsafe {
        device.cmd_copy_buffer_to_image(
            command.command_buffer,
            staging.buffer,
            texture.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[copy],
        );
    }
    let to_shader = vk::ImageMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ)
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(texture.image)
        .subresource_range(range);
    unsafe {
        device.cmd_pipeline_barrier(
            command.command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[to_shader],
        );
    }
    unsafe { device.end_command_buffer(command.command_buffer) }
        .map_err(|error| format!("failed to end texture upload command buffer: {error}"))?;
    let commands = [command.command_buffer];
    let submits = [vk::SubmitInfo::default().command_buffers(&commands)];
    let _queue_guard = inner.lock_queue();
    unsafe { device.queue_submit(inner.queue(), &submits, vk::Fence::null()) }
        .map_err(|error| format!("failed to submit texture upload: {error}"))?;
    unsafe { device.queue_wait_idle(inner.queue()) }
        .map_err(|error| format!("failed waiting for texture upload: {error}"))?;
    Ok(())
}

fn color_subresource_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
}

fn find_memory_type(
    inner: &DeviceInner,
    type_bits: u32,
    required: vk::MemoryPropertyFlags,
    preferred: vk::MemoryPropertyFlags,
) -> Result<u32, String> {
    let properties = unsafe {
        inner
            .instance()
            .get_physical_device_memory_properties(inner.physical_device())
    };
    let mut fallback = None;
    for index in 0..properties.memory_type_count {
        if type_bits & (1 << index) == 0 {
            continue;
        }
        let flags = properties.memory_types[index as usize].property_flags;
        if !flags.contains(required) {
            continue;
        }
        if !preferred.is_empty() && flags.contains(preferred) {
            return Ok(index);
        }
        fallback.get_or_insert(index);
    }
    fallback.ok_or_else(|| "Vulkan device has no compatible texture memory type".into())
}
