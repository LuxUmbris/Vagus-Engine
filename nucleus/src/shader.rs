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
use ash::vk;
use std::sync::Arc;

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

#[derive(Clone)]
pub struct ShaderDevice {
    inner: Arc<SharedDevice>,
}

pub struct ShaderModule {
    inner: Arc<SharedDevice>,
    handle: vk::ShaderModule,
}

impl ShaderDevice {
    pub(crate) fn from_shared(inner: Arc<SharedDevice>) -> Self {
        Self { inner }
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

impl ShaderModule {
    pub fn belongs_to_device(&self, device: &ShaderDevice) -> bool {
        Arc::ptr_eq(&self.inner, &device.inner)
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        unsafe {
            self.inner.device.destroy_shader_module(self.handle, None);
        }
    }
}
