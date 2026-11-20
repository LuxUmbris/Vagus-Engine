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
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// An owned, tightly packed RGBA8 texture image.
///
/// Image-file decoding is intentionally separate; this type accepts explicit
/// pixel data so its layout and lifetime are unambiguous.
#[derive(Clone, Debug)]
pub struct Texture {
    width: u32,
    height: u32,
    rgba8: Arc<Vec<u8>>,
    content_hash: u64,
    fully_opaque: bool,
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height && self.rgba8 == other.rgba8
    }
}

impl Eq for Texture {}

impl Hash for Texture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.width.hash(state);
        self.height.hash(state);
        self.content_hash.hash(state);
    }
}

impl Texture {
    pub fn from_rgba8(width: u32, height: u32, pixels: &[u8]) -> Result<Self, String> {
        nucleus::validate_rgba8_texture_data(width, height, pixels)?;
        let fully_opaque = pixels.chunks_exact(4).all(|pixel| pixel[3] == u8::MAX);
        let mut rgba8 = Vec::new();
        rgba8
            .try_reserve_exact(pixels.len())
            .map_err(|error| format!("could not allocate texture pixels: {error}"))?;
        rgba8.extend_from_slice(pixels);
        let mut hasher = DefaultHasher::new();
        width.hash(&mut hasher);
        height.hash(&mut hasher);
        rgba8.hash(&mut hasher);
        Ok(Self {
            width,
            height,
            rgba8: Arc::new(rgba8),
            content_hash: hasher.finish(),
            fully_opaque,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba8(&self) -> &[u8] {
        self.rgba8.as_slice()
    }

    /// Returns whether every texel has an alpha value of 255.
    pub fn is_fully_opaque(&self) -> bool {
        self.fully_opaque
    }
}

/// Creates GPU-resident textures using a Vulkan graphics device.
#[derive(Clone)]
pub struct TextureDevice {
    inner: nucleus::TextureDevice,
}

/// A Vulkan RGBA8 image with an image view and sampler, owned by the device.
#[derive(Clone)]
pub struct GpuTexture {
    inner: nucleus::GpuTexture,
}

impl TextureDevice {
    pub(crate) fn from_backend(inner: nucleus::TextureDevice) -> Self {
        Self { inner }
    }

    /// Creates a standalone Vulkan device for texture uploads.
    /// Use `Renderer::texture_device` to share the presentation device.
    pub fn new_standalone() -> Result<Self, String> {
        Ok(Self {
            inner: nucleus::TextureDevice::new_standalone()?,
        })
    }

    pub fn upload(&self, texture: &Texture) -> Result<GpuTexture, String> {
        Ok(GpuTexture {
            inner: self.inner.create_rgba8_texture(
                texture.width,
                texture.height,
                texture.rgba8.as_slice(),
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

    pub(crate) fn backend_clone(&self) -> nucleus::GpuTexture {
        self.inner.clone()
    }
}
