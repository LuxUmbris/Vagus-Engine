//! Portable public audio output API.
//!
//! Writes use interleaved floating-point samples in the range `-1.0..=1.0`. The return
//! value is accepted frames, not samples; short writes and zero indicate backpressure.
//! Retry the unaccepted suffix. `drain` is an explicit completion barrier and may wait.

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

/// Portable output format and latency request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioConfig {
    /// Output sample rate in samples per second.
    pub sample_rate: u32,
    /// Number of interleaved channels. The portable API supports mono and stereo.
    pub channels: u16,
    /// Requested output latency in microseconds. Backends may treat this as a hint.
    pub latency_micros: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            latency_micros: 40_000,
        }
    }
}

impl AudioConfig {
    /// Validates settings shared by the supported output backends.
    pub fn validate(self) -> Result<(), String> {
        if !(8_000..=192_000).contains(&self.sample_rate) {
            return Err("audio sample rate must be between 8000 and 192000 Hz".into());
        }
        if !(1..=2).contains(&self.channels) {
            return Err("portable audio output supports one or two channels".into());
        }
        if !(1_000..=2_000_000).contains(&self.latency_micros) {
            return Err("audio latency must be between 1000 and 2000000 microseconds".into());
        }
        Ok(())
    }
}

/// Owns a native audio output device.
///
/// Writes do not wait for playback to finish. Use [`drain`](Self::drain) when completion
/// of queued audio is required.
pub struct AudioOutput {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    backend: nucleus::audio::AudioOutput,
    config: AudioConfig,
}

impl AudioOutput {
    /// Opens the system's default output device.
    pub fn open(config: AudioConfig) -> Result<Self, String> {
        Self::open_device("default", config)
    }

    /// Opens a named ALSA PCM on Linux or a numeric WinMM device ID on Windows.
    pub fn open_device(device: &str, config: AudioConfig) -> Result<Self, String> {
        config.validate()?;

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let backend_config = nucleus::audio::AudioConfig {
                sample_rate: config.sample_rate,
                channels: config.channels,
                latency_micros: config.latency_micros,
            };
            let backend = nucleus::audio::AudioOutput::open_device(device, backend_config)?;
            Ok(Self { backend, config })
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = device;
            Err("audio output is unsupported on this target".into())
        }
    }

    /// Returns the configuration used to open this output.
    pub fn config(&self) -> AudioConfig {
        self.config
    }

    /// Submits interleaved float samples and returns accepted frames.
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<usize, String> {
        if !samples
            .len()
            .is_multiple_of(usize::from(self.config.channels))
        {
            return Err(format!(
                "sample count {} is not divisible by channel count {}",
                samples.len(),
                self.config.channels
            ));
        }

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            self.backend.write_samples(samples)
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = samples;
            Err("audio output is unsupported on this target".into())
        }
    }

    /// Waits for queued output to complete and prepares the device for subsequent writes.
    pub fn drain(&mut self) -> Result<(), String> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            self.backend.drain()
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err("audio output is unsupported on this target".into())
        }
    }
}
