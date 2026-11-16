use std::ffi::c_void;

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
    _entry: ash::Entry,
    instance: Option<ash::Instance>,
    surface_loader: Option<ash::khr::surface::Instance>,
    surface: ash::vk::SurfaceKHR,
    device: Option<ash::Device>,
    swapchain_loader: Option<ash::khr::swapchain::Device>,
    swapchain: ash::vk::SwapchainKHR,
    physical_device: ash::vk::PhysicalDevice,
    queue_family: u32,
    queue: ash::vk::Queue,
    command_pool: ash::vk::CommandPool,
    command_buffer: ash::vk::CommandBuffer,
    image_available: ash::vk::Semaphore,
    render_finished: ash::vk::Semaphore,
    fence: ash::vk::Fence,
    staging_buffer: ash::vk::Buffer,
    staging_memory: ash::vk::DeviceMemory,
    staging_size: ash::vk::DeviceSize,
    extent: ash::vk::Extent2D,
    format: ash::vk::Format,
}

impl VulkanRenderer {
    pub fn new(window: NativeWindow, width: u32, height: u32) -> Result<Self, String> {
        let entry = unsafe { ash::Entry::load() }
            .map_err(|error| format!("loading the Vulkan loader failed: {error}"))?;
        let mut renderer = Self {
            _entry: entry,
            instance: None,
            surface_loader: None,
            surface: ash::vk::SurfaceKHR::null(),
            device: None,
            swapchain_loader: None,
            swapchain: ash::vk::SwapchainKHR::null(),
            physical_device: ash::vk::PhysicalDevice::null(),
            queue_family: 0,
            queue: ash::vk::Queue::null(),
            command_pool: ash::vk::CommandPool::null(),
            command_buffer: ash::vk::CommandBuffer::null(),
            image_available: ash::vk::Semaphore::null(),
            render_finished: ash::vk::Semaphore::null(),
            fence: ash::vk::Fence::null(),
            staging_buffer: ash::vk::Buffer::null(),
            staging_memory: ash::vk::DeviceMemory::null(),
            staging_size: 0,
            extent: ash::vk::Extent2D::default(),
            format: ash::vk::Format::UNDEFINED,
        };
        renderer.initialize(window, width, height)?;
        Ok(renderer)
    }

    fn initialize(&mut self, window: NativeWindow, width: u32, height: u32) -> Result<(), String> {
        use ash::vk;
        let app_name = std::ffi::CString::new("Vagus Engine").unwrap();
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(&app_name)
            .api_version(vk::API_VERSION_1_0);
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
            unsafe { self._entry.create_instance(&create_info, None) }
                .map_err(|error| format!("creating Vulkan instance failed: {error:?}"))?,
        );
        let instance = self.instance.as_ref().unwrap();
        self.surface_loader = Some(ash::khr::surface::Instance::new(&self._entry, instance));
        #[cfg(target_os = "linux")]
        {
            self.surface = match window {
                NativeWindow::Xlib { display, window } => {
                    let loader = ash::khr::xlib_surface::Instance::new(&self._entry, instance);
                    let info = vk::XlibSurfaceCreateInfoKHR::default()
                        .dpy(display.cast())
                        .window(window as vk::Window);
                    unsafe { loader.create_xlib_surface(&info, None) }.map_err(|error| {
                        format!("creating the Vulkan Xlib surface failed: {error:?}")
                    })?
                }
                NativeWindow::Wayland { display, surface } => {
                    let loader = ash::khr::wayland_surface::Instance::new(&self._entry, instance);
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
            let loader = ash::khr::win32_surface::Instance::new(&self._entry, instance);
            let info = vk::Win32SurfaceCreateInfoKHR::default()
                .hinstance(hinstance as isize)
                .hwnd(hwnd as isize);
            self.surface = unsafe { loader.create_win32_surface(&info, None) }
                .map_err(|error| format!("creating the Vulkan Win32 surface failed: {error:?}"))?;
        }
        self.select_device()?;
        self.create_swapchain(width, height)?;
        self.create_commands()?;
        self.create_staging_buffer()?;
        Ok(())
    }

    fn select_device(&mut self) -> Result<(), String> {
        use ash::vk;
        let instance = self.instance.as_ref().unwrap();
        let surface_loader = self.surface_loader.as_ref().unwrap();
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
                    self.physical_device = physical;
                    self.queue_family = index as u32;
                    break;
                }
            }
            if self.physical_device != vk::PhysicalDevice::null() {
                break;
            }
        }
        if self.physical_device == vk::PhysicalDevice::null() {
            return Err("no Vulkan graphics device supports presentation to this window".into());
        }
        let priorities = [1.0];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(self.queue_family)
            .queue_priorities(&priorities)];
        let device_extensions = [ash::khr::swapchain::NAME.as_ptr()];
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_extension_names(&device_extensions);
        self.device = Some(
            unsafe { instance.create_device(self.physical_device, &device_info, None) }
                .map_err(|error| format!("creating Vulkan device failed: {error:?}"))?,
        );
        self.queue = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .get_device_queue(self.queue_family, 0)
        };
        self.swapchain_loader = Some(ash::khr::swapchain::Device::new(
            instance,
            self.device.as_ref().unwrap(),
        ));
        Ok(())
    }

    fn create_swapchain(&mut self, width: u32, height: u32) -> Result<(), String> {
        use ash::vk;
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let surface_loader = self.surface_loader.as_ref().unwrap();
        let caps = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
        }
        .map_err(|error| format!("querying Vulkan surface capabilities failed: {error:?}"))?;
        if !caps
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_DST)
        {
            return Err("Vulkan surface images do not support transfer destinations".into());
        }
        let formats = unsafe {
            surface_loader.get_physical_device_surface_formats(self.physical_device, self.surface)
        }
        .map_err(|error| format!("querying Vulkan surface formats failed: {error:?}"))?;
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
            .image_usage(vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite)
            .present_mode(vk::PresentModeKHR::FIFO)
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
            unsafe {
                self.swapchain_loader
                    .as_ref()
                    .unwrap()
                    .destroy_swapchain(self.swapchain, None)
            };
        }
        self.swapchain = new_swapchain;
        let _ = unsafe {
            self.swapchain_loader
                .as_ref()
                .unwrap()
                .get_swapchain_images(self.swapchain)
        }
        .map_err(|error| format!("getting Vulkan swapchain images failed: {error:?}"))?;
        let _ = instance;
        let _ = device;
        Ok(())
    }

    fn create_commands(&mut self) -> Result<(), String> {
        use ash::vk;
        let device = self.device.as_ref().unwrap();
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
        self.render_finished =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .map_err(|error| format!("creating Vulkan present semaphore failed: {error:?}"))?;
        self.fence = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }
        .map_err(|error| format!("creating Vulkan fence failed: {error:?}"))?;
        Ok(())
    }

    fn create_staging_buffer(&mut self) -> Result<(), String> {
        use ash::vk;
        let bytes = (self.extent.width as u64)
            .checked_mul(self.extent.height as u64)
            .and_then(|size| size.checked_mul(4))
            .ok_or_else(|| "Vulkan staging buffer size overflow".to_owned())?;
        let device = self.device.as_ref().unwrap();
        let info = vk::BufferCreateInfo::default()
            .size(bytes)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        self.staging_buffer = unsafe { device.create_buffer(&info, None) }
            .map_err(|error| format!("creating Vulkan staging buffer failed: {error:?}"))?;
        let requirements = unsafe { device.get_buffer_memory_requirements(self.staging_buffer) };
        let memory_properties = unsafe {
            self.instance
                .as_ref()
                .unwrap()
                .get_physical_device_memory_properties(self.physical_device)
        };
        let memory_type = (0..memory_properties.memory_type_count)
            .find(|index| {
                let flags = memory_properties.memory_types[*index as usize].property_flags;
                requirements.memory_type_bits & (1 << index) != 0
                    && flags.contains(
                        vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT,
                    )
            })
            .ok_or_else(|| {
                "Vulkan device has no host-visible coherent staging memory".to_owned()
            })?;
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        self.staging_memory = unsafe { device.allocate_memory(&alloc, None) }
            .map_err(|error| format!("allocating Vulkan staging memory failed: {error:?}"))?;
        unsafe { device.bind_buffer_memory(self.staging_buffer, self.staging_memory, 0) }
            .map_err(|error| format!("binding Vulkan staging memory failed: {error:?}"))?;
        self.staging_size = bytes;
        Ok(())
    }

    pub fn present(&mut self, pixels: &[[u8; 3]], width: u32, height: u32) -> Result<(), String> {
        use ash::vk;
        if width == 0 || height == 0 {
            return Ok(());
        }
        if width != self.extent.width || height != self.extent.height {
            unsafe { self.device.as_ref().unwrap().device_wait_idle() }
                .map_err(|error| format!("waiting before Vulkan resize failed: {error:?}"))?;
            if self.staging_buffer != vk::Buffer::null() {
                unsafe {
                    self.device
                        .as_ref()
                        .unwrap()
                        .destroy_buffer(self.staging_buffer, None)
                };
                unsafe {
                    self.device
                        .as_ref()
                        .unwrap()
                        .free_memory(self.staging_memory, None)
                };
                self.staging_buffer = vk::Buffer::null();
                self.staging_memory = vk::DeviceMemory::null();
            }
            self.create_swapchain(width, height)?;
            self.create_staging_buffer()?;
        }
        let expected = self.extent.width as usize * self.extent.height as usize;
        if pixels.len() != expected {
            return Err("pixel buffer does not match Vulkan swapchain extent".into());
        }
        let mut rgba = Vec::with_capacity(expected * 4);
        let bgra = matches!(
            self.format,
            vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
        );
        for [red, green, blue] in pixels {
            if bgra {
                rgba.extend_from_slice(&[*blue, *green, *red, 255]);
            } else {
                rgba.extend_from_slice(&[*red, *green, *blue, 255]);
            }
        }
        let device = self.device.as_ref().unwrap();
        let mapped = unsafe {
            device.map_memory(
                self.staging_memory,
                0,
                self.staging_size,
                vk::MemoryMapFlags::empty(),
            )
        }
        .map_err(|error| format!("mapping Vulkan staging memory failed: {error:?}"))?;
        unsafe {
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), mapped.cast::<u8>(), rgba.len());
            device.unmap_memory(self.staging_memory);
        }
        let (image_index, _) = unsafe {
            self.swapchain_loader.as_ref().unwrap().acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        }
        .map_err(|error| format!("acquiring Vulkan swapchain image failed: {error:?}"))?;
        unsafe { device.wait_for_fences(&[self.fence], true, u64::MAX) }
            .map_err(|error| format!("waiting for Vulkan frame fence failed: {error:?}"))?;
        unsafe { device.reset_fences(&[self.fence]) }
            .map_err(|error| format!("resetting Vulkan frame fence failed: {error:?}"))?;
        unsafe {
            device.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
        }
        .map_err(|error| format!("resetting Vulkan command buffer failed: {error:?}"))?;
        let begin = vk::CommandBufferBeginInfo::default();
        unsafe { device.begin_command_buffer(self.command_buffer, &begin) }
            .map_err(|error| format!("beginning Vulkan command buffer failed: {error:?}"))?;
        let images = unsafe {
            self.swapchain_loader
                .as_ref()
                .unwrap()
                .get_swapchain_images(self.swapchain)
        }
        .map_err(|error| format!("getting Vulkan swapchain images failed: {error:?}"))?;
        let image = images[image_index as usize];
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
        unsafe {
            device.cmd_copy_buffer_to_image(
                self.command_buffer,
                self.staging_buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copy],
            );
        }
        let to_present = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(range);
        unsafe {
            device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TRANSFER,
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
        let wait_stages = [vk::PipelineStageFlags::TRANSFER];
        let commands = [self.command_buffer];
        let signals = [self.render_finished];
        let submit = [vk::SubmitInfo::default()
            .wait_semaphores(&waits)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&commands)
            .signal_semaphores(&signals)];
        unsafe { device.queue_submit(self.queue, &submit, self.fence) }
            .map_err(|error| format!("submitting Vulkan frame failed: {error:?}"))?;
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
        Ok(())
    }
}

/// Platform-neutral window presenter used by the public rendering API.
pub struct Renderer(VulkanRenderer);

impl Renderer {
    pub fn new(window: &crate::WindowHandle, width: u32, height: u32) -> Result<Self, String> {
        Ok(Self(VulkanRenderer::new(
            native_window(window),
            width,
            height,
        )?))
    }

    pub fn present(&mut self, pixels: &[[u8; 3]], width: u32, height: u32) -> Result<(), String> {
        self.0.present(pixels, width, height)
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            if let Some(device) = &self.device {
                let _ = device.device_wait_idle();
                if self.staging_buffer != ash::vk::Buffer::null() {
                    device.destroy_buffer(self.staging_buffer, None);
                }
                if self.staging_memory != ash::vk::DeviceMemory::null() {
                    device.free_memory(self.staging_memory, None);
                }
                if self.fence != ash::vk::Fence::null() {
                    device.destroy_fence(self.fence, None);
                }
                if self.render_finished != ash::vk::Semaphore::null() {
                    device.destroy_semaphore(self.render_finished, None);
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
                device.destroy_device(None);
            }
            if self.surface != ash::vk::SurfaceKHR::null() {
                if let Some(loader) = &self.surface_loader {
                    loader.destroy_surface(self.surface, None);
                }
            }
            if let Some(instance) = &self.instance {
                instance.destroy_instance(None);
            }
        }
    }
}
