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

//! Deterministic noise helpers for procedural generation.

use super::heightmap::HeightMap;

/// Parameters for normalized fractal Brownian motion (fBm) value noise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseSettings {
    pub seed: u64,
    /// World-space distance between the lowest-frequency lattice points.
    pub scale: f32,
    pub octaves: u32,
    /// Amplitude multiplier for each successive octave, in `0.0..=1.0`.
    pub persistence: f32,
    /// Frequency multiplier for each successive octave.
    pub lacunarity: f32,
}

impl Default for NoiseSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            scale: 32.0,
            octaves: 4,
            persistence: 0.5,
            lacunarity: 2.0,
        }
    }
}

impl NoiseSettings {
    fn validate(self) -> Result<(), String> {
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err("noise scale must be finite and positive".to_owned());
        }
        if !(1..=32).contains(&self.octaves) {
            return Err("noise octaves must be between 1 and 32".to_owned());
        }
        if !self.persistence.is_finite() || !(0.0..=1.0).contains(&self.persistence) {
            return Err("noise persistence must be finite and in 0..=1".to_owned());
        }
        if !self.lacunarity.is_finite() || self.lacunarity <= 0.0 {
            return Err("noise lacunarity must be finite and positive".to_owned());
        }
        Ok(())
    }
}

/// Samples deterministic smooth value noise in `[-1, 1]` at world coordinates.
pub fn sample_value_noise_2d(x: f32, y: f32, seed: u64) -> Result<f32, String> {
    if !x.is_finite() || !y.is_finite() {
        return Err("noise coordinates must be finite".to_owned());
    }
    value_noise(f64::from(x), f64::from(y), seed).map(|value| value as f32)
}

/// Samples normalized fractal Brownian motion at world coordinates.
///
/// The lowest frequency has a lattice spacing of `settings.scale` world units.
pub fn sample_fbm(x: f32, y: f32, settings: NoiseSettings) -> Result<f32, String> {
    settings.validate()?;
    if !x.is_finite() || !y.is_finite() {
        return Err("noise coordinates must be finite".to_owned());
    }

    let mut frequency = 1.0_f64;
    let mut amplitude = 1.0_f64;
    let mut total = 0.0_f64;
    let mut amplitude_total = 0.0_f64;
    for octave in 0..settings.octaves {
        let sample_x = f64::from(x) / f64::from(settings.scale) * frequency;
        let sample_y = f64::from(y) / f64::from(settings.scale) * frequency;
        if !sample_x.is_finite() || !sample_y.is_finite() {
            return Err("noise coordinates exceed the supported lattice range".to_owned());
        }
        let octave_seed = settings.seed.wrapping_add(u64::from(octave));
        total += value_noise(sample_x, sample_y, octave_seed)? * amplitude;
        amplitude_total += amplitude;
        frequency *= f64::from(settings.lacunarity);
        amplitude *= f64::from(settings.persistence);
    }

    Ok((total / amplitude_total) as f32)
}

/// Builds a row-major heightmap by sampling fBm at each grid coordinate.
pub fn generate_heightmap(
    width: u32,
    height: u32,
    settings: NoiseSettings,
) -> Result<HeightMap, String> {
    settings.validate()?;
    HeightMap::try_from_fn(width, height, |x, y| {
        sample_fbm(x as f32, y as f32, settings)
    })
}

fn value_noise(x: f64, y: f64, seed: u64) -> Result<f64, String> {
    let x_floor = x.floor();
    let y_floor = y.floor();
    if x_floor < i64::MIN as f64
        || x_floor >= i64::MAX as f64
        || y_floor < i64::MIN as f64
        || y_floor >= i64::MAX as f64
    {
        return Err("noise coordinates exceed the supported lattice range".to_owned());
    }

    let x0 = x_floor as i64;
    let y0 = y_floor as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = fade(x - x_floor);
    let ty = fade(y - y_floor);
    let top = lerp(lattice_value(x0, y0, seed), lattice_value(x1, y0, seed), tx);
    let bottom = lerp(lattice_value(x0, y1, seed), lattice_value(x1, y1, seed), tx);
    Ok(lerp(top, bottom, ty))
}

fn lattice_value(x: i64, y: i64, seed: u64) -> f64 {
    let mut hash = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B1_85EB_CA87)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 31;
    ((hash >> 40) as f64 / 8_388_607.5) - 1.0
}

fn fade(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, amount: f64) -> f64 {
    a + (b - a) * amount
}
