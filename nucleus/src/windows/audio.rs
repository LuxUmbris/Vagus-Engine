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

use std::ffi::{c_char, c_void};
use std::ptr;
use std::thread;
use std::time::Duration;

const WAVE_MAPPER: u32 = u32::MAX;
const WAVE_FORMAT_PCM: u16 = 1;
const WHDR_DONE: u32 = 1;

#[repr(C)]
struct WaveFormatEx {
    format_tag: u16,
    channels: u16,
    samples_per_sec: u32,
    avg_bytes_per_sec: u32,
    block_align: u16,
    bits_per_sample: u16,
    extra_size: u16,
}

#[repr(C)]
struct WaveHeader {
    data: *mut c_char,
    buffer_length: u32,
    bytes_recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut WaveHeader,
    reserved: usize,
}

struct PendingBuffer {
    header: Box<WaveHeader>,
    _samples: Box<[i16]>,
}

#[link(name = "winmm")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn waveOutOpen(
        output: *mut *mut c_void,
        device_id: u32,
        format: *const WaveFormatEx,
        callback: usize,
        instance: usize,
        flags: u32,
    ) -> u32;
    fn waveOutPrepareHeader(output: *mut c_void, header: *mut WaveHeader, size: u32) -> u32;
    fn waveOutWrite(output: *mut c_void, header: *mut WaveHeader, size: u32) -> u32;
    fn waveOutUnprepareHeader(output: *mut c_void, header: *mut WaveHeader, size: u32) -> u32;
    fn waveOutReset(output: *mut c_void) -> u32;
    fn waveOutClose(output: *mut c_void) -> u32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioConfig {
    /// Output sample rate in samples per second.
    pub sample_rate: u32,
    /// Number of interleaved channels (WinMM supports mono or stereo here).
    pub channels: u16,
    /// Accepted for API parity; WinMM queues caller-sized blocks asynchronously.
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

pub struct AudioOutput {
    handle: *mut c_void,
    config: AudioConfig,
    pending: Vec<PendingBuffer>,
}

impl AudioOutput {
    pub fn open(config: AudioConfig) -> Result<Self, String> {
        Self::open_device("default", config)
    }

    /// Opens the default output device, or a numbered WinMM device.
    pub fn open_device(device: &str, config: AudioConfig) -> Result<Self, String> {
        validate_config(config)?;
        let device_id = if device == "default" {
            WAVE_MAPPER
        } else {
            device
                .parse::<u32>()
                .map_err(|_| "Windows audio device must be `default` or a numeric WinMM ID")?
        };
        let block_align = config.channels * 2;
        let format = WaveFormatEx {
            format_tag: WAVE_FORMAT_PCM,
            channels: config.channels,
            samples_per_sec: config.sample_rate,
            avg_bytes_per_sec: config.sample_rate * u32::from(block_align),
            block_align,
            bits_per_sample: 16,
            extra_size: 0,
        };
        let mut handle = ptr::null_mut();
        let result = unsafe { waveOutOpen(&mut handle, device_id, &format, 0, 0, 0) };
        if result != 0 {
            return Err(format!("waveOutOpen failed with WinMM error {result}"));
        }
        Ok(Self {
            handle,
            config,
            pending: Vec::new(),
        })
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    /// Queues interleaved float samples without waiting for playback to finish.
    /// Returns the number of frames queued, or zero while a previous block is pending.
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<usize, String> {
        let channels = usize::from(self.config.channels);
        if !samples.len().is_multiple_of(channels) {
            return Err(format!(
                "sample count {} is not divisible by the {} channel count",
                samples.len(),
                channels
            ));
        }
        if samples.is_empty() {
            return Ok(0);
        }
        self.reap_completed()?;
        if !self.pending.is_empty() {
            return Ok(0);
        }
        let byte_len = samples
            .len()
            .checked_mul(std::mem::size_of::<i16>())
            .filter(|&bytes| bytes <= u32::MAX as usize)
            .ok_or_else(|| "audio block is too large for WinMM".to_owned())?;
        let mut pcm: Box<[i16]> = samples
            .iter()
            .copied()
            .map(float_to_pcm)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut header = Box::new(WaveHeader {
            data: pcm.as_mut_ptr().cast(),
            buffer_length: byte_len as u32,
            bytes_recorded: 0,
            user: 0,
            flags: 0,
            loops: 0,
            next: ptr::null_mut(),
            reserved: 0,
        });
        let header_size = std::mem::size_of::<WaveHeader>() as u32;
        let prepare = unsafe { waveOutPrepareHeader(self.handle, &mut *header, header_size) };
        if prepare != 0 {
            return Err(format!(
                "waveOutPrepareHeader failed with WinMM error {prepare}"
            ));
        }
        let write = unsafe { waveOutWrite(self.handle, &mut *header, header_size) };
        if write != 0 {
            unsafe {
                waveOutUnprepareHeader(self.handle, &mut *header, header_size);
            }
            return Err(format!("waveOutWrite failed with WinMM error {write}"));
        }

        let frames = samples.len() / channels;
        self.pending.push(PendingBuffer {
            header,
            _samples: pcm,
        });
        Ok(frames)
    }

    fn reap_completed(&mut self) -> Result<(), String> {
        let header_size = std::mem::size_of::<WaveHeader>() as u32;
        let mut index = 0;
        while index < self.pending.len() {
            let header = &mut *self.pending[index].header as *mut WaveHeader;
            let flags = unsafe { ptr::read_volatile(ptr::addr_of!((*header).flags)) };
            if flags & WHDR_DONE == 0 {
                index += 1;
                continue;
            }
            let result = unsafe { waveOutUnprepareHeader(self.handle, header, header_size) };
            if result != 0 {
                return Err(format!(
                    "waveOutUnprepareHeader failed with WinMM error {result}"
                ));
            }
            self.pending.swap_remove(index);
        }
        Ok(())
    }

    /// Waits for queued buffers to finish and releases their storage.
    pub fn drain(&mut self) -> Result<(), String> {
        while !self.pending.is_empty() {
            self.reap_completed()?;
            if !self.pending.is_empty() {
                thread::sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        unsafe {
            waveOutReset(self.handle);
            let header_size = std::mem::size_of::<WaveHeader>() as u32;
            for buffer in &mut self.pending {
                waveOutUnprepareHeader(self.handle, &mut *buffer.header, header_size);
            }
            self.pending.clear();
            waveOutClose(self.handle);
        }
    }
}

fn validate_config(config: AudioConfig) -> Result<(), String> {
    if !(8_000..=192_000).contains(&config.sample_rate) {
        return Err("sample rate must be between 8 kHz and 192 kHz".into());
    }
    if !(1..=2).contains(&config.channels) {
        return Err("WinMM output supports one or two channels".into());
    }
    if !(1_000..=2_000_000).contains(&config.latency_micros) {
        return Err("latency must be between 1 ms and 2 s".into());
    }
    Ok(())
}

fn float_to_pcm(sample: f32) -> i16 {
    let sample = if sample.is_finite() { sample } else { 0.0 }.clamp(-1.0, 1.0);
    let scaled = if sample < 0.0 {
        sample * 32_768.0
    } else {
        sample * 32_767.0
    };
    scaled.round() as i16
}

#[cfg(test)]
mod tests {
    use super::{AudioConfig, float_to_pcm, validate_config};

    #[test]
    fn validates_config_and_pcm_conversion() {
        assert!(validate_config(AudioConfig::default()).is_ok());
        assert!(
            validate_config(AudioConfig {
                channels: 3,
                ..AudioConfig::default()
            })
            .is_err()
        );
        assert_eq!(float_to_pcm(-1.0), i16::MIN);
        assert_eq!(float_to_pcm(1.0), i16::MAX);
    }
}
