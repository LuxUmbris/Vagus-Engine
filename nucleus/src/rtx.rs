use ash::{Device, Entry, Instance, vk};
use std::{ffi::CString, ptr};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    pub vertices: [[f32; 3]; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub origin: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub vertical_fov_degrees: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            origin: [0.0, 0.0, 2.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            vertical_fov_degrees: 60.0,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Buffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: u64,
    address: vk::DeviceAddress,
}

pub fn hardware_ray_tracing_supported() -> bool {
    let Ok(entry) = (unsafe { Entry::load() }) else {
        return false;
    };
    let app = CString::new("Vagus RTX support probe").unwrap();
    let ai = vk::ApplicationInfo::default()
        .application_name(&app)
        .api_version(vk::API_VERSION_1_2);
    let Ok(instance) = (unsafe {
        entry.create_instance(
            &vk::InstanceCreateInfo::default().application_info(&ai),
            None,
        )
    }) else {
        return false;
    };
    let yes = unsafe { instance.enumerate_physical_devices() }
        .unwrap_or_default()
        .iter()
        .any(|p| {
            let ex =
                unsafe { instance.enumerate_device_extension_properties(*p) }.unwrap_or_default();
            let has = |n: &std::ffi::CStr| {
                ex.iter()
                    .any(|e| unsafe { std::ffi::CStr::from_ptr(e.extension_name.as_ptr()) == n })
            };
            if !has(ash::khr::acceleration_structure::NAME)
                || !has(ash::khr::ray_tracing_pipeline::NAME)
                || !has(ash::khr::deferred_host_operations::NAME)
            {
                return false;
            }
            let mut r = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
            let mut a = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
            let mut b = vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
            let mut f = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut r)
                .push_next(&mut a)
                .push_next(&mut b);
            unsafe { instance.get_physical_device_features2(*p, &mut f) };
            r.ray_tracing_pipeline == vk::TRUE
                && a.acceleration_structure == vk::TRUE
                && b.buffer_device_address == vk::TRUE
        });
    unsafe { instance.destroy_instance(None) };
    yes
}

pub struct RtxRenderer {
    _entry: Entry,
    instance: Instance,
    physical: vk::PhysicalDevice,
    device: Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    rt: ash::khr::ray_tracing_pipeline::Device,
    as_loader: ash::khr::acceleration_structure::Device,
    width: u32,
    height: u32,
    output: Buffer,
    vertices: Buffer,
    blas_buffer: Buffer,
    blas: vk::AccelerationStructureKHR,
    instances: Buffer,
    tlas_buffer: Buffer,
    tlas: vk::AccelerationStructureKHR,
    scratch: Buffer,
    sbt: Buffer,
    sbt_offset: u64,
    descriptor_pool: vk::DescriptorPool,
    descriptor_layout: vk::DescriptorSetLayout,
    descriptor_set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}
impl RtxRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("ray tracing target dimensions must be non-zero".into());
        }
        let pixels = (width as usize)
            .checked_mul(height as usize)
            .filter(|n| *n <= u32::MAX as usize / 4)
            .ok_or("ray tracing target is too large")?;
        let entry =
            unsafe { Entry::load() }.map_err(|e| format!("loading Vulkan loader failed: {e}"))?;
        let app = CString::new("Vagus nucleus ray tracer").unwrap();
        let ai = vk::ApplicationInfo::default()
            .application_name(&app)
            .engine_name(&app)
            .api_version(vk::API_VERSION_1_2);
        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(&ai),
                None,
            )
        }
        .map_err(|e| format!("creating Vulkan instance failed: {e:?}"))?;
        let mut selected = None;
        for p in unsafe { instance.enumerate_physical_devices() }
            .map_err(|e| format!("enumerating devices failed: {e:?}"))?
        {
            let ex = unsafe { instance.enumerate_device_extension_properties(p) }
                .map_err(|e| format!("enumerating extensions failed: {e:?}"))?;
            let has = |n: &std::ffi::CStr| {
                ex.iter()
                    .any(|e| unsafe { std::ffi::CStr::from_ptr(e.extension_name.as_ptr()) == n })
            };
            if !has(ash::khr::acceleration_structure::NAME)
                || !has(ash::khr::ray_tracing_pipeline::NAME)
                || !has(ash::khr::deferred_host_operations::NAME)
            {
                continue;
            }
            let mut r = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
            let mut a = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
            let mut b = vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
            let mut f = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut r)
                .push_next(&mut a)
                .push_next(&mut b);
            unsafe { instance.get_physical_device_features2(p, &mut f) };
            if r.ray_tracing_pipeline != vk::TRUE
                || a.acceleration_structure != vk::TRUE
                || b.buffer_device_address != vk::TRUE
            {
                continue;
            }
            if let Some((i, _)) = unsafe { instance.get_physical_device_queue_family_properties(p) }
                .iter()
                .enumerate()
                .find(|(_, q)| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            {
                selected = Some((p, i as u32));
                break;
            }
        }
        let Some((p, fam)) = selected else {
            unsafe { instance.destroy_instance(None) };
            return Err(
                "no Vulkan device supports KHR ray tracing pipeline and acceleration structures"
                    .into(),
            );
        };
        let pri = [1.0];
        let qi = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(fam)
            .queue_priorities(&pri)];
        let exts = [
            ash::khr::deferred_host_operations::NAME.as_ptr(),
            ash::khr::acceleration_structure::NAME.as_ptr(),
            ash::khr::ray_tracing_pipeline::NAME.as_ptr(),
        ];
        let mut r =
            vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default().ray_tracing_pipeline(true);
        let mut a = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default()
            .acceleration_structure(true);
        let mut b =
            vk::PhysicalDeviceBufferDeviceAddressFeatures::default().buffer_device_address(true);
        let di = vk::DeviceCreateInfo::default()
            .queue_create_infos(&qi)
            .enabled_extension_names(&exts)
            .push_next(&mut r)
            .push_next(&mut a)
            .push_next(&mut b);
        let device = match unsafe { instance.create_device(p, &di, None) } {
            Ok(device) => device,
            Err(error) => {
                unsafe { instance.destroy_instance(None) };
                return Err(format!("creating ray tracing device failed: {error:?}"));
            }
        };
        let queue = unsafe { device.get_device_queue(fam, 0) };
        let cp = match unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(fam)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        } {
            Ok(pool) => pool,
            Err(error) => {
                unsafe {
                    device.destroy_device(None);
                    instance.destroy_instance(None);
                }
                return Err(format!("creating command pool failed: {error:?}"));
            }
        };
        let rt = ash::khr::ray_tracing_pipeline::Device::new(&instance, &device);
        let as_loader = ash::khr::acceleration_structure::Device::new(&instance, &device);
        let mut s = Self {
            _entry: entry,
            instance,
            physical: p,
            device,
            queue,
            command_pool: cp,
            rt,
            as_loader,
            width,
            height,
            output: Buffer::default(),
            vertices: Buffer::default(),
            blas_buffer: Buffer::default(),
            blas: vk::AccelerationStructureKHR::null(),
            instances: Buffer::default(),
            tlas_buffer: Buffer::default(),
            tlas: vk::AccelerationStructureKHR::null(),
            scratch: Buffer::default(),
            sbt: Buffer::default(),
            sbt_offset: 0,
            descriptor_pool: vk::DescriptorPool::null(),
            descriptor_layout: vk::DescriptorSetLayout::null(),
            descriptor_set: vk::DescriptorSet::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
        };
        s.initialize(pixels)?;
        Ok(s)
    }
    fn initialize(&mut self, pixels: usize) -> Result<(), String> {
        self.output = self.create_buffer(
            (pixels * 4) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            true,
            false,
        )?;
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR),
        ];
        self.descriptor_layout = unsafe {
            self.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .map_err(|e| format!("creating ray tracing descriptor layout failed: {e:?}"))?;
        let pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                .descriptor_count(1),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1),
        ];
        self.descriptor_pool = unsafe {
            self.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&pool_sizes),
                None,
            )
        }
        .map_err(|e| format!("creating ray tracing descriptor pool failed: {e:?}"))?;
        let layouts = [self.descriptor_layout];
        self.descriptor_set = unsafe {
            self.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(self.descriptor_pool)
                    .set_layouts(&layouts),
            )
        }
        .map_err(|e| format!("allocating ray tracing descriptor set failed: {e:?}"))?[0];
        let push = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR)
            .offset(0)
            .size(std::mem::size_of::<CameraPush>() as u32);
        self.pipeline_layout = unsafe {
            self.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts)
                    .push_constant_ranges(std::slice::from_ref(&push)),
                None,
            )
        }
        .map_err(|e| format!("creating ray tracing pipeline layout failed: {e:?}"))?;

        let raygen = shader_module(&self.device, include_bytes!("shaders/spv/raygen.spv"))?;
        let miss = match shader_module(&self.device, include_bytes!("shaders/spv/miss.spv")) {
            Ok(m) => m,
            Err(e) => {
                unsafe { self.device.destroy_shader_module(raygen, None) };
                return Err(e);
            }
        };
        let hit = match shader_module(&self.device, include_bytes!("shaders/spv/hit.spv")) {
            Ok(m) => m,
            Err(e) => {
                unsafe {
                    self.device.destroy_shader_module(raygen, None);
                    self.device.destroy_shader_module(miss, None)
                };
                return Err(e);
            }
        };
        let entry = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::RAYGEN_KHR)
                .module(raygen)
                .name(&entry),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::MISS_KHR)
                .module(miss)
                .name(&entry),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                .module(hit)
                .name(&entry),
        ];
        let groups = [
            vk::RayTracingShaderGroupCreateInfoKHR::default()
                .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                .general_shader(0)
                .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                .any_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR),
            vk::RayTracingShaderGroupCreateInfoKHR::default()
                .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                .general_shader(1)
                .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                .any_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR),
            vk::RayTracingShaderGroupCreateInfoKHR::default()
                .ty(vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP)
                .general_shader(vk::SHADER_UNUSED_KHR)
                .closest_hit_shader(2)
                .any_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR),
        ];
        let pipeline_result = unsafe {
            self.rt.create_ray_tracing_pipelines(
                vk::DeferredOperationKHR::null(),
                vk::PipelineCache::null(),
                &[vk::RayTracingPipelineCreateInfoKHR::default()
                    .stages(&stages)
                    .groups(&groups)
                    .max_pipeline_ray_recursion_depth(1)
                    .layout(self.pipeline_layout)],
                None,
            )
        };
        unsafe {
            self.device.destroy_shader_module(raygen, None);
            self.device.destroy_shader_module(miss, None);
            self.device.destroy_shader_module(hit, None)
        };
        self.pipeline = pipeline_result
            .map_err(|(_, e)| format!("creating ray tracing pipeline failed: {e:?}"))?[0];
        let mut rt_properties = vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::default();
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut rt_properties);
        unsafe {
            self.instance
                .get_physical_device_properties2(self.physical, &mut properties)
        };
        let handle_size = rt_properties.shader_group_handle_size as u64;
        let handle_alignment = rt_properties.shader_group_handle_alignment as u64;
        let base_alignment = rt_properties.shader_group_base_alignment as u64;
        let stride = align_up(handle_size, handle_alignment)?;
        let raygen_offset = 0;
        let miss_offset = align_up(raygen_offset + stride, base_alignment)?;
        let hit_offset = align_up(miss_offset + stride, base_alignment)?;
        let total = hit_offset
            .checked_add(stride)
            .ok_or("shader binding table size overflow")?;
        let allocation_size = total
            .checked_add(base_alignment)
            .ok_or("shader binding table allocation size overflow")?;
        self.sbt = self.create_buffer(
            allocation_size,
            vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR,
            true,
            true,
        )?;
        self.sbt_offset = align_up(self.sbt.address, base_alignment)? - self.sbt.address;
        let handles = unsafe {
            self.rt.get_ray_tracing_shader_group_handles(
                self.pipeline,
                0,
                3,
                (handle_size * 3) as usize,
            )
        }
        .map_err(|e| format!("reading ray tracing shader handles failed: {e:?}"))?;
        let mut table = vec![0u8; allocation_size as usize];
        for (index, offset) in [raygen_offset, miss_offset, hit_offset]
            .into_iter()
            .enumerate()
        {
            let start = index * handle_size as usize;
            let offset = self.sbt_offset + offset;
            table[offset as usize..offset as usize + handle_size as usize]
                .copy_from_slice(&handles[start..start + handle_size as usize]);
        }
        self.write(self.sbt, &table)?;
        Ok(())
    }
    fn create_buffer(
        &self,
        n: u64,
        usage: vk::BufferUsageFlags,
        host: bool,
        address: bool,
    ) -> Result<Buffer, String> {
        let usage = usage
            | if address {
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
            } else {
                vk::BufferUsageFlags::empty()
            };
        let b = unsafe {
            self.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(n.max(4))
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .map_err(|e| format!("creating buffer failed: {e:?}"))?;
        let req = unsafe { self.device.get_buffer_memory_requirements(b) };
        let mp = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical)
        };
        let want = if host {
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        } else {
            vk::MemoryPropertyFlags::DEVICE_LOCAL
        };
        let mt = (0..mp.memory_type_count)
            .find(|i| {
                req.memory_type_bits & (1 << i) != 0
                    && mp.memory_types[*i as usize].property_flags.contains(want)
            })
            .ok_or("no compatible Vulkan memory type")?;
        let mut alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(mt);
        let mut flags =
            vk::MemoryAllocateFlagsInfo::default().flags(vk::MemoryAllocateFlags::DEVICE_ADDRESS);
        if address {
            alloc = alloc.push_next(&mut flags)
        }
        let mem = unsafe { self.device.allocate_memory(&alloc, None) }
            .map_err(|e| format!("allocating buffer memory failed: {e:?}"))?;
        unsafe { self.device.bind_buffer_memory(b, mem, 0) }
            .map_err(|e| format!("binding buffer failed: {e:?}"))?;
        let addr = if address {
            unsafe {
                self.device
                    .get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(b))
            }
        } else {
            0
        };
        Ok(Buffer {
            buffer: b,
            memory: mem,
            size: n.max(4),
            address: addr,
        })
    }
    fn write<T>(&self, b: Buffer, data: &[T]) -> Result<(), String> {
        let n = std::mem::size_of_val(data) as u64;
        let p = unsafe {
            self.device
                .map_memory(b.memory, 0, n, vk::MemoryMapFlags::empty())
        }
        .map_err(|e| format!("mapping memory failed: {e:?}"))?;
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr().cast::<u8>(), p.cast::<u8>(), n as usize);
            self.device.unmap_memory(b.memory)
        };
        Ok(())
    }
    pub fn trace(
        &mut self,
        triangles: &[Triangle],
        camera: Camera,
    ) -> Result<Vec<[u8; 4]>, String> {
        trace_scene(self, triangles, camera)
    }
}
impl Drop for RtxRenderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            if self.pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.pipeline, None)
            }
            if self.pipeline_layout != vk::PipelineLayout::null() {
                self.device
                    .destroy_pipeline_layout(self.pipeline_layout, None)
            }
            if self.descriptor_pool != vk::DescriptorPool::null() {
                self.device
                    .destroy_descriptor_pool(self.descriptor_pool, None)
            }
            if self.descriptor_layout != vk::DescriptorSetLayout::null() {
                self.device
                    .destroy_descriptor_set_layout(self.descriptor_layout, None)
            }
            for a in [self.tlas, self.blas] {
                if a != vk::AccelerationStructureKHR::null() {
                    self.as_loader.destroy_acceleration_structure(a, None)
                }
            }
            for b in [
                self.vertices,
                self.blas_buffer,
                self.instances,
                self.tlas_buffer,
                self.output,
                self.scratch,
                self.sbt,
            ] {
                if b.buffer != vk::Buffer::null() {
                    self.device.destroy_buffer(b.buffer, None)
                }
                if b.memory != vk::DeviceMemory::null() {
                    self.device.free_memory(b.memory, None)
                }
            }
            if self.command_pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(self.command_pool, None)
            }
            self.device.destroy_device(None);
            self.instance.destroy_instance(None)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CameraPush {
    origin: [f32; 4],
    forward: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    tan_half_fov: f32,
    width: u32,
    height: u32,
    _padding: u32,
}

fn align_up(value: u64, alignment: u64) -> Result<u64, String> {
    if alignment == 0 {
        return Err("Vulkan reported zero alignment".into());
    }
    value
        .checked_add(alignment - 1)
        .map(|v| v / alignment * alignment)
        .ok_or_else(|| "alignment overflow".into())
}

fn shader_module(device: &Device, bytes: &[u8]) -> Result<vk::ShaderModule, String> {
    if bytes.len() % 4 != 0 {
        return Err("embedded SPIR-V byte length is not a multiple of four".into());
    }
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    unsafe {
        device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
    }
    .map_err(|e| format!("creating embedded shader module failed: {e:?}"))
}

fn trace_scene(
    renderer: &mut RtxRenderer,
    triangles: &[Triangle],
    camera: Camera,
) -> Result<Vec<[u8; 4]>, String> {
    if triangles.is_empty() {
        return Err("ray tracing scene must contain at least one triangle".into());
    }
    if triangles.len() > u32::MAX as usize / 3 {
        return Err("ray tracing scene has too many triangles".into());
    }
    if !camera.vertical_fov_degrees.is_finite()
        || camera.vertical_fov_degrees <= 0.0
        || camera.vertical_fov_degrees >= 179.0
    {
        return Err("camera vertical field of view must be between 0 and 179 degrees".into());
    }
    if triangles
        .iter()
        .flat_map(|t| t.vertices.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err("triangle vertices must be finite".into());
    }

    unsafe { renderer.device.device_wait_idle() }
        .map_err(|e| format!("waiting for prior ray tracing work failed: {e:?}"))?;
    for handle in [&mut renderer.tlas, &mut renderer.blas] {
        if *handle != vk::AccelerationStructureKHR::null() {
            unsafe {
                renderer
                    .as_loader
                    .destroy_acceleration_structure(*handle, None)
            };
            *handle = vk::AccelerationStructureKHR::null();
        }
    }
    destroy_buffer(&renderer.device, &mut renderer.vertices);
    destroy_buffer(&renderer.device, &mut renderer.blas_buffer);
    destroy_buffer(&renderer.device, &mut renderer.instances);
    destroy_buffer(&renderer.device, &mut renderer.tlas_buffer);
    destroy_buffer(&renderer.device, &mut renderer.scratch);

    let vertex_count = triangles.len() * 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    for triangle in triangles {
        vertices.extend_from_slice(&triangle.vertices);
    }
    renderer.vertices = renderer.create_buffer(
        std::mem::size_of_val(vertices.as_slice()) as u64,
        vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        true,
        true,
    )?;
    renderer.write(renderer.vertices, &vertices)?;

    let triangles_data = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
        .vertex_format(vk::Format::R32G32B32_SFLOAT)
        .vertex_data(vk::DeviceOrHostAddressConstKHR {
            device_address: renderer.vertices.address,
        })
        .vertex_stride(std::mem::size_of::<[f32; 3]>() as u64)
        .max_vertex(vertex_count as u32 - 1)
        .index_type(vk::IndexType::NONE_KHR);
    let blas_geometry = vk::AccelerationStructureGeometryKHR::default()
        .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
        .geometry(vk::AccelerationStructureGeometryDataKHR {
            triangles: triangles_data,
        })
        .flags(vk::GeometryFlagsKHR::OPAQUE);
    let primitive_counts = [triangles.len() as u32];
    let mut blas_build = vk::AccelerationStructureBuildGeometryInfoKHR::default()
        .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
        .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
        .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
        .geometries(std::slice::from_ref(&blas_geometry));
    let mut blas_sizes = vk::AccelerationStructureBuildSizesInfoKHR::default();
    unsafe {
        renderer.as_loader.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &blas_build,
            &primitive_counts,
            &mut blas_sizes,
        )
    };
    renderer.blas_buffer = renderer.create_buffer(
        blas_sizes.acceleration_structure_size,
        vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR,
        false,
        true,
    )?;
    renderer.blas = unsafe {
        renderer.as_loader.create_acceleration_structure(
            &vk::AccelerationStructureCreateInfoKHR::default()
                .buffer(renderer.blas_buffer.buffer)
                .size(blas_sizes.acceleration_structure_size)
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL),
            None,
        )
    }
    .map_err(|e| format!("creating bottom-level acceleration structure failed: {e:?}"))?;

    let blas_address = unsafe {
        renderer
            .as_loader
            .get_acceleration_structure_device_address(
                &vk::AccelerationStructureDeviceAddressInfoKHR::default()
                    .acceleration_structure(renderer.blas),
            )
    };
    let instance = vk::AccelerationStructureInstanceKHR {
        transform: vk::TransformMatrixKHR {
            matrix: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        },
        instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xff),
        instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, 0),
        acceleration_structure_reference: vk::AccelerationStructureReferenceKHR {
            device_handle: blas_address,
        },
    };
    renderer.instances = renderer.create_buffer(
        std::mem::size_of::<vk::AccelerationStructureInstanceKHR>() as u64,
        vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        true,
        true,
    )?;
    renderer.write(renderer.instances, std::slice::from_ref(&instance))?;
    let instances_data = vk::AccelerationStructureGeometryInstancesDataKHR::default()
        .array_of_pointers(false)
        .data(vk::DeviceOrHostAddressConstKHR {
            device_address: renderer.instances.address,
        });
    let tlas_geometry = vk::AccelerationStructureGeometryKHR::default()
        .geometry_type(vk::GeometryTypeKHR::INSTANCES)
        .geometry(vk::AccelerationStructureGeometryDataKHR {
            instances: instances_data,
        });
    let tlas_primitive_counts = [1];
    let mut tlas_build = vk::AccelerationStructureBuildGeometryInfoKHR::default()
        .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
        .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
        .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
        .geometries(std::slice::from_ref(&tlas_geometry));
    let mut tlas_sizes = vk::AccelerationStructureBuildSizesInfoKHR::default();
    unsafe {
        renderer.as_loader.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &tlas_build,
            &tlas_primitive_counts,
            &mut tlas_sizes,
        )
    };
    renderer.tlas_buffer = renderer.create_buffer(
        tlas_sizes.acceleration_structure_size,
        vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR,
        false,
        true,
    )?;
    renderer.tlas = unsafe {
        renderer.as_loader.create_acceleration_structure(
            &vk::AccelerationStructureCreateInfoKHR::default()
                .buffer(renderer.tlas_buffer.buffer)
                .size(tlas_sizes.acceleration_structure_size)
                .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL),
            None,
        )
    }
    .map_err(|e| format!("creating top-level acceleration structure failed: {e:?}"))?;

    let mut acceleration_properties =
        vk::PhysicalDeviceAccelerationStructurePropertiesKHR::default();
    let mut device_properties =
        vk::PhysicalDeviceProperties2::default().push_next(&mut acceleration_properties);
    unsafe {
        renderer
            .instance
            .get_physical_device_properties2(renderer.physical, &mut device_properties)
    };
    let scratch_alignment =
        acceleration_properties.min_acceleration_structure_scratch_offset_alignment as u64;
    let scratch_size = blas_sizes
        .build_scratch_size
        .max(tlas_sizes.build_scratch_size)
        .checked_add(scratch_alignment)
        .ok_or("acceleration-structure scratch buffer size overflow")?;
    renderer.scratch = renderer.create_buffer(
        scratch_size,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        false,
        true,
    )?;
    let scratch_address = align_up(renderer.scratch.address, scratch_alignment)?;
    blas_build = blas_build
        .dst_acceleration_structure(renderer.blas)
        .scratch_data(vk::DeviceOrHostAddressKHR {
            device_address: scratch_address,
        });
    tlas_build = tlas_build
        .dst_acceleration_structure(renderer.tlas)
        .scratch_data(vk::DeviceOrHostAddressKHR {
            device_address: scratch_address,
        });

    let mut acceleration_write = vk::WriteDescriptorSetAccelerationStructureKHR::default()
        .acceleration_structures(std::slice::from_ref(&renderer.tlas));
    let as_write = [vk::WriteDescriptorSet::default()
        .dst_set(renderer.descriptor_set)
        .dst_binding(0)
        .descriptor_count(1)
        .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
        .push_next(&mut acceleration_write)];
    let output_info = [vk::DescriptorBufferInfo::default()
        .buffer(renderer.output.buffer)
        .offset(0)
        .range(renderer.output.size)];
    let output_write = [vk::WriteDescriptorSet::default()
        .dst_set(renderer.descriptor_set)
        .dst_binding(1)
        .descriptor_count(1)
        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        .buffer_info(&output_info)];
    unsafe { renderer.device.update_descriptor_sets(&as_write, &[]) };
    unsafe { renderer.device.update_descriptor_sets(&output_write, &[]) };

    let direction = normalize(sub(camera.target, camera.origin))?;
    let right = normalize(cross(direction, camera.up))?;
    let up = normalize(cross(right, direction))?;
    let push = CameraPush {
        origin: [camera.origin[0], camera.origin[1], camera.origin[2], 0.0],
        forward: [direction[0], direction[1], direction[2], 0.0],
        right: [right[0], right[1], right[2], 0.0],
        up: [up[0], up[1], up[2], 0.0],
        tan_half_fov: (camera.vertical_fov_degrees.to_radians() * 0.5).tan(),
        width: renderer.width,
        height: renderer.height,
        _padding: 0,
    };

    let command = unsafe {
        renderer.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(renderer.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )
    }
    .map_err(|e| format!("allocating ray tracing command buffer failed: {e:?}"))?[0];
    let result: Result<(), String> = (|| {
        unsafe {
            renderer.device.begin_command_buffer(
                command,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        }
        .map_err(|e| format!("beginning ray tracing command buffer failed: {e:?}"))?;
        let blas_range = [vk::AccelerationStructureBuildRangeInfoKHR::default()
            .primitive_count(triangles.len() as u32)];
        unsafe {
            renderer.as_loader.cmd_build_acceleration_structures(
                command,
                std::slice::from_ref(&blas_build),
                &[&blas_range],
            )
        };
        let build_barrier = [vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR)
            .dst_access_mask(
                vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
                    | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
            )];
        unsafe {
            renderer.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::DependencyFlags::empty(),
                &build_barrier,
                &[],
                &[],
            )
        };
        let tlas_range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(1)];
        unsafe {
            renderer.as_loader.cmd_build_acceleration_structures(
                command,
                std::slice::from_ref(&tlas_build),
                &[&tlas_range],
            )
        };
        let trace_barrier = [vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR)
            .dst_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR)];
        unsafe {
            renderer.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
                vk::DependencyFlags::empty(),
                &trace_barrier,
                &[],
                &[],
            );
            renderer.device.cmd_bind_pipeline(
                command,
                vk::PipelineBindPoint::RAY_TRACING_KHR,
                renderer.pipeline,
            );
            renderer.device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::RAY_TRACING_KHR,
                renderer.pipeline_layout,
                0,
                &[renderer.descriptor_set],
                &[],
            );
            renderer.device.cmd_push_constants(
                command,
                renderer.pipeline_layout,
                vk::ShaderStageFlags::RAYGEN_KHR,
                0,
                std::slice::from_raw_parts(
                    (&push as *const CameraPush).cast::<u8>(),
                    std::mem::size_of::<CameraPush>(),
                ),
            );
        }
        let mut ray_properties = vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::default();
        let mut properties =
            vk::PhysicalDeviceProperties2::default().push_next(&mut ray_properties);
        unsafe {
            renderer
                .instance
                .get_physical_device_properties2(renderer.physical, &mut properties)
        };
        let handle_size = ray_properties.shader_group_handle_size as u64;
        let handle_alignment = ray_properties.shader_group_handle_alignment as u64;
        let base_alignment = ray_properties.shader_group_base_alignment as u64;
        let stride = align_up(handle_size, handle_alignment)?;
        let miss_offset = align_up(stride, base_alignment)?;
        let hit_offset = align_up(miss_offset + stride, base_alignment)?;
        let raygen = vk::StridedDeviceAddressRegionKHR {
            device_address: renderer.sbt.address + renderer.sbt_offset,
            stride,
            size: stride,
        };
        let miss = vk::StridedDeviceAddressRegionKHR {
            device_address: renderer.sbt.address + renderer.sbt_offset + miss_offset,
            stride,
            size: stride,
        };
        let hit = vk::StridedDeviceAddressRegionKHR {
            device_address: renderer.sbt.address + renderer.sbt_offset + hit_offset,
            stride,
            size: stride,
        };
        let callable = vk::StridedDeviceAddressRegionKHR::default();
        unsafe {
            renderer.rt.cmd_trace_rays(
                command,
                &raygen,
                &miss,
                &hit,
                &callable,
                renderer.width,
                renderer.height,
                1,
            )
        };
        let host_barrier = [vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::HOST_READ)];
        unsafe {
            renderer.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &host_barrier,
                &[],
                &[],
            );
            renderer.device.end_command_buffer(command)
        }
        .map_err(|e| format!("ending ray tracing command buffer failed: {e:?}"))?;
        let commands = [command];
        let submits = [vk::SubmitInfo::default().command_buffers(&commands)];
        unsafe {
            renderer
                .device
                .queue_submit(renderer.queue, &submits, vk::Fence::null())
        }
        .map_err(|e| format!("submitting ray tracing work failed: {e:?}"))?;
        unsafe { renderer.device.queue_wait_idle(renderer.queue) }
            .map_err(|e| format!("waiting for ray tracing work failed: {e:?}"))?;
        Ok(())
    })();
    unsafe {
        renderer
            .device
            .free_command_buffers(renderer.command_pool, &[command])
    };
    result?;

    let byte_count = renderer.width as usize * renderer.height as usize * 4;
    let mapped = unsafe {
        renderer.device.map_memory(
            renderer.output.memory,
            0,
            byte_count as u64,
            vk::MemoryMapFlags::empty(),
        )
    }
    .map_err(|e| format!("mapping ray tracing output failed: {e:?}"))?;
    let mut pixels = Vec::with_capacity(renderer.width as usize * renderer.height as usize);
    let words = mapped.cast::<u32>();
    for index in 0..renderer.width as usize * renderer.height as usize {
        pixels.push(unsafe { words.add(index).read_unaligned() }.to_le_bytes());
    }
    unsafe { renderer.device.unmap_memory(renderer.output.memory) };
    Ok(pixels)
}

fn destroy_buffer(device: &Device, buffer: &mut Buffer) {
    unsafe {
        if buffer.buffer != vk::Buffer::null() {
            device.destroy_buffer(buffer.buffer, None);
        }
        if buffer.memory != vk::DeviceMemory::null() {
            device.free_memory(buffer.memory, None);
        }
    }
    *buffer = Buffer::default();
}

fn normalize(v: [f32; 3]) -> Result<[f32; 3], String> {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        return Err("camera direction and up vectors must be non-zero and finite".into());
    }
    Ok([v[0] / length, v[1] / length, v[2] / length])
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
