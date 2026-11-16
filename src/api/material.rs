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

use super::heightmap;
use super::texture;

/// A CPU-side terrain material pairing an image texture with elevation data.
pub struct Material {
    texture: texture::Texture,
    heightmap: heightmap::HeightMap,
}

impl Material {
    pub fn new(texture: texture::Texture, heightmap: heightmap::HeightMap) -> Self {
        Self { texture, heightmap }
    }

    pub fn texture(&self) -> &texture::Texture {
        &self.texture
    }

    pub fn heightmap(&self) -> &heightmap::HeightMap {
        &self.heightmap
    }

    pub fn into_parts(self) -> (texture::Texture, heightmap::HeightMap) {
        (self.texture, self.heightmap)
    }
}
