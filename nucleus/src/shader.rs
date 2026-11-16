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

use ash::{Device, Entry, Instance, vk};
use std::{ffi::CString, sync::Arc};

const SPIRV_MAGIC: u32 = 0x0723_0203;
const SPIRV_MAX_MINOR: u32 = 6;

pub fn validate_spirv_words(words: &[u32]) -> Result<(), String> {
    if words.len() < 6 {
        return Err("SPIR-V module must contain a header and at least one instruction".into());
    }
    if words[0] != SPIRV_MAGIC {
        return Err("invalid SPIR-V magic number".into());
    }

    let version = words[1];
    let major = (version >> 16) & 0xff;
    let minor = (version >> 8) & 0xff;
    if version & 0xff00_00ff != 0 || major != 1 || minor > SPIRV_MAX_MINOR {
        return Err(format!(
            "unsupported or malformed SPIR-V version 0x{version:08x}"
        ));
    }
    if words[3] == 0 {
        return Err("SPIR-V ID bound must be nonzero".into());
    }
    if words[4] != 0 {
        return Err("SPIR-V schema must be zero".into());
    }

    let mut offset = 5;
    while offset < words.len() {
        let instruction_words = (words[offset] >> 16) as usize;
        if instruction_words == 0 {
            return Err(format!(
                "SPIR-V instruction at word {offset} has zero length"
            ));
        }
        if instruction_words > words.len() - offset {
            return Err(format!(
                "SPIR-V instruction at word {offset} exceeds the module"
            ));
        }
        offset += instruction_words;
    }
    Ok(())
}

struct DeviceInner {
    _entry: Entry,
    instance: Instance,
    device: Device,
}

impl Drop for DeviceInner {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

#[derive(Clone)]
pub struct ShaderDevice {
    inner: Arc<DeviceInner>,
}

pub struct ShaderModule {
    inner: Arc<DeviceInner>,
    handle: vk::ShaderModule,
}

impl ShaderDevice {
    pub fn new() -> Result<Self, String> {
        let entry =
            unsafe { Entry::load() }.map_err(|error| format!("failed to load Vulkan: {error}"))?;
        let app_name = CString::new("Vagus shader device").expect("static string has no NUL");
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(&app_name)
            .api_version(vk::API_VERSION_1_0);
        let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);
        let instance = unsafe { entry.create_instance(&create_info, None) }
            .map_err(|error| format!("failed to create Vulkan instance: {error}"))?;

        let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
            Ok(devices) => devices,
            Err(error) => {
                unsafe { instance.destroy_instance(None) };
                return Err(format!("failed to enumerate Vulkan devices: {error}"));
            }
        };
        let selection = physical_devices.into_iter().find_map(|physical| {
            unsafe { instance.get_physical_device_queue_family_properties(physical) }
                .iter()
                .position(|family| family.queue_count > 0)
                .map(|index| (physical, index as u32))
        });
        let Some((physical, queue_family)) = selection else {
            unsafe { instance.destroy_instance(None) };
            return Err("no Vulkan device with a usable queue family was found".into());
        };

        let priorities = [1.0_f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_info);
        let device = match unsafe { instance.create_device(physical, &device_info, None) } {
            Ok(device) => device,
            Err(error) => {
                unsafe { instance.destroy_instance(None) };
                return Err(format!("failed to create Vulkan shader device: {error}"));
            }
        };

        Ok(Self {
            inner: Arc::new(DeviceInner {
                _entry: entry,
                instance,
                device,
            }),
        })
    }

    pub fn create_shader_module(&self, words: &[u32]) -> Result<ShaderModule, String> {
        validate_spirv_words(words)?;
        let info = vk::ShaderModuleCreateInfo::default().code(words);
        let handle = unsafe { self.inner.device.create_shader_module(&info, None) }
            .map_err(|error| format!("Vulkan rejected the SPIR-V shader module: {error}"))?;
        Ok(ShaderModule {
            inner: Arc::clone(&self.inner),
            handle,
        })
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        unsafe {
            self.inner.device.destroy_shader_module(self.handle, None);
        }
    }
}
