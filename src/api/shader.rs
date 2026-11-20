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

use std::ffi::CString;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

/// A precompiled SPIR-V module with a requested shader stage and entry point.
///
/// Module construction checks the SPIR-V container and declared entry point; the
/// Vulkan driver performs the final module validation when it is loaded.
pub struct Shader {
    stage: ShaderStage,
    entry_point: String,
    words: Vec<u32>,
}

impl Shader {
    pub fn from_spirv(
        stage: ShaderStage,
        words: &[u32],
        entry_point: &str,
    ) -> Result<Self, String> {
        nucleus::validate_spirv_words(words)?;
        validate_entry_point(words, stage, entry_point)?;
        Ok(Self {
            stage,
            entry_point: entry_point.to_owned(),
            words: words.to_vec(),
        })
    }

    pub fn from_spirv_bytes(
        stage: ShaderStage,
        bytes: &[u8],
        entry_point: &str,
    ) -> Result<Self, String> {
        if bytes.len() % 4 != 0 {
            return Err("SPIR-V bytecode length must be a multiple of four".into());
        }
        let words: Vec<_> = bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        Self::from_spirv(stage, &words, entry_point)
    }

    pub fn stage(&self) -> ShaderStage {
        self.stage
    }

    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }

    pub fn words(&self) -> &[u32] {
        &self.words
    }
}

/// Loads shader modules on a renderer's shared Vulkan device.
///
/// Obtain this handle from `Renderer::shader_device`. This API accepts precompiled SPIR-V; it does
/// not compile GLSL/HLSL. Loaded vertex and fragment modules can be installed with the corresponding
/// renderer methods.
#[derive(Clone)]
pub struct ShaderDevice {
    inner: nucleus::ShaderDevice,
}

pub struct ShaderModule {
    _inner: nucleus::ShaderModule,
    stage: ShaderStage,
    entry_point: String,
    words: Vec<u32>,
}

impl ShaderDevice {
    pub(crate) fn from_shared(inner: nucleus::ShaderDevice) -> Self {
        Self { inner }
    }

    pub fn load(&self, shader: &Shader) -> Result<ShaderModule, String> {
        Ok(ShaderModule {
            _inner: self.inner.create_shader_module(shader.words())?,
            stage: shader.stage(),
            entry_point: shader.entry_point().to_owned(),
            words: shader.words().to_vec(),
        })
    }
}

impl ShaderModule {
    pub fn stage(&self) -> ShaderStage {
        self.stage
    }

    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }

    pub(crate) fn belongs_to_device(&self, device: &nucleus::ShaderDevice) -> bool {
        self._inner.belongs_to_device(device)
    }

    pub(crate) fn words(&self) -> &[u32] {
        &self.words
    }
}

fn validate_entry_point(
    words: &[u32],
    stage: ShaderStage,
    entry_point: &str,
) -> Result<(), String> {
    if entry_point.is_empty() {
        return Err("shader entry point must not be empty".into());
    }
    CString::new(entry_point)
        .map_err(|_| String::from("shader entry point must not contain NUL"))?;

    let execution_model = match stage {
        ShaderStage::Vertex => 0,
        ShaderStage::Fragment => 4,
        ShaderStage::Compute => 5,
    };
    let mut offset = 5;
    while offset < words.len() {
        let instruction = words[offset];
        let word_count = (instruction >> 16) as usize;
        if instruction as u16 == 15 && word_count >= 4 {
            let string_words = &words[offset + 3..offset + word_count];
            let mut name = Vec::new();
            let mut terminated = false;
            for word in string_words {
                for byte in word.to_le_bytes() {
                    if byte == 0 {
                        terminated = true;
                        break;
                    }
                    name.push(byte);
                }
                if terminated {
                    break;
                }
            }
            if terminated && words[offset + 1] == execution_model && name == entry_point.as_bytes()
            {
                return Ok(());
            }
        }
        offset += word_count;
    }
    Err(format!(
        "SPIR-V module has no {stage:?} entry point named {entry_point:?}"
    ))
}
