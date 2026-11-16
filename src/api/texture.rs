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

/// An owned, tightly packed RGBA8 texture image.
///
/// Image-file decoding is intentionally separate; this type accepts explicit
/// pixel data so its layout and lifetime are unambiguous.
pub struct Texture {
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

impl Texture {
    pub fn from_rgba8(width: u32, height: u32, pixels: &[u8]) -> Result<Self, String> {
        nucleus::validate_rgba8_texture_data(width, height, pixels)?;
        let mut rgba8 = Vec::new();
        rgba8
            .try_reserve_exact(pixels.len())
            .map_err(|error| format!("could not allocate texture pixels: {error}"))?;
        rgba8.extend_from_slice(pixels);
        Ok(Self {
            width,
            height,
            rgba8,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }
}

/// Creates GPU-resident textures using a Vulkan graphics device.
#[derive(Clone)]
pub struct TextureDevice {
    inner: nucleus::TextureDevice,
}

/// A Vulkan RGBA8 image with an image view and sampler, owned by the device.
pub struct GpuTexture {
    inner: nucleus::GpuTexture,
}

impl TextureDevice {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: nucleus::TextureDevice::new()?,
        })
    }

    pub fn upload(&self, texture: &Texture) -> Result<GpuTexture, String> {
        Ok(GpuTexture {
            inner: self.inner.create_rgba8_texture(
                texture.width,
                texture.height,
                &texture.rgba8,
            )?,
        })
    }
}

impl GpuTexture {
    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    pub fn height(&self) -> u32 {
        self.inner.height()
    }
}
