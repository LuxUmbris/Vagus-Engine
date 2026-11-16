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

/// A row-major grid of finite elevation samples.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightMap {
    width: u32,
    height: u32,
    samples: Vec<f32>,
}

impl HeightMap {
    /// Creates a heightmap from row-major samples.
    pub fn new(width: u32, height: u32, samples: Vec<f32>) -> Result<Self, String> {
        let expected = sample_count(width, height)?;
        if samples.len() != expected {
            return Err(format!(
                "heightmap has {} samples; expected {expected}",
                samples.len()
            ));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err("heightmap samples must be finite".to_owned());
        }

        Ok(Self {
            width,
            height,
            samples,
        })
    }

    /// Generates samples in row-major order, passing `(x, y)` grid coordinates.
    pub fn from_fn(
        width: u32,
        height: u32,
        mut sample: impl FnMut(u32, u32) -> f32,
    ) -> Result<Self, String> {
        Self::try_from_fn(width, height, |x, y| Ok(sample(x, y)))
    }

    /// Generates samples while preserving errors returned by a custom generator.
    pub fn try_from_fn(
        width: u32,
        height: u32,
        mut sample: impl FnMut(u32, u32) -> Result<f32, String>,
    ) -> Result<Self, String> {
        let count = sample_count(width, height)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(count)
            .map_err(|error| format!("could not allocate heightmap samples: {error}"))?;

        for y in 0..height {
            for x in 0..width {
                let value = sample(x, y)?;
                if !value.is_finite() {
                    return Err(format!("heightmap sample at ({x}, {y}) is not finite"));
                }
                samples.push(value);
            }
        }

        Ok(Self {
            width,
            height,
            samples,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns all elevation samples in row-major order.
    pub fn heights(&self) -> &[f32] {
        &self.samples
    }

    /// Returns the sample at an integer grid coordinate.
    pub fn get(&self, x: u32, y: u32) -> Option<f32> {
        self.index(x, y).map(|index| self.samples[index])
    }

    /// Replaces one sample. Non-finite elevations are rejected.
    pub fn set(&mut self, x: u32, y: u32, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("heightmap samples must be finite".to_owned());
        }
        let index = self
            .index(x, y)
            .ok_or_else(|| format!("heightmap coordinate ({x}, {y}) is out of bounds"))?;
        self.samples[index] = value;
        Ok(())
    }

    /// Samples at floating-point grid coordinates using bilinear interpolation.
    /// Coordinates outside the grid or non-finite coordinates return `None`.
    pub fn sample_bilinear(&self, x: f32, y: f32) -> Option<f32> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x > (self.width - 1) as f32
            || y > (self.height - 1) as f32
        {
            return None;
        }

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = x0.saturating_add(1).min(self.width - 1);
        let y1 = y0.saturating_add(1).min(self.height - 1);
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let top = self.get(x0, y0)? * (1.0 - tx) + self.get(x1, y0)? * tx;
        let bottom = self.get(x0, y1)? * (1.0 - tx) + self.get(x1, y1)? * tx;
        Some(top * (1.0 - ty) + bottom * ty)
    }

    pub fn min_max(&self) -> (f32, f32) {
        self.samples
            .iter()
            .copied()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            })
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(y)
            .ok()?
            .checked_mul(usize::try_from(self.width).ok()?)?
            .checked_add(usize::try_from(x).ok()?)
    }
}

fn sample_count(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 {
        return Err("heightmap dimensions must be non-zero".to_owned());
    }

    usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok()?.checked_mul(width))
        .ok_or_else(|| "heightmap dimensions overflow address space".to_owned())
}
