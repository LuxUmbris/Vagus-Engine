use std::ffi::{CStr, CString, c_char, c_int, c_long, c_ulong, c_void};
use std::ptr;

#[repr(C)]
struct SndPcm {
    _private: [u8; 0],
}

type SndPcmFrames = c_long;
type SndPcmUFrames = c_ulong;
const SND_PCM_NONBLOCK: c_int = 1;

#[link(name = "asound")]
unsafe extern "C" {
    fn snd_pcm_open(
        pcm: *mut *mut SndPcm,
        name: *const c_char,
        stream: c_int,
        mode: c_int,
    ) -> c_int;
    fn snd_pcm_close(pcm: *mut SndPcm) -> c_int;
    fn snd_pcm_drop(pcm: *mut SndPcm) -> c_int;
    fn snd_pcm_drain(pcm: *mut SndPcm) -> c_int;
    fn snd_pcm_prepare(pcm: *mut SndPcm) -> c_int;
    fn snd_pcm_writei(pcm: *mut SndPcm, buffer: *const c_void, size: SndPcmUFrames)
    -> SndPcmFrames;
    fn snd_pcm_recover(pcm: *mut SndPcm, error: c_int, silent: c_int) -> c_int;
    fn snd_pcm_set_params(
        pcm: *mut SndPcm,
        format: c_int,
        access: c_int,
        channels: u32,
        rate: u32,
        soft_resample: c_int,
        latency: u32,
    ) -> c_int;
    fn snd_strerror(error: c_int) -> *const c_char;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    /// ALSA requested buffer latency, in microseconds.
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
    pcm: *mut SndPcm,
    config: AudioConfig,
}

impl AudioOutput {
    pub fn open(config: AudioConfig) -> Result<Self, String> {
        Self::open_device("default", config)
    }

    /// Opens a named ALSA PCM device, such as `default` or `null`.
    pub fn open_device(device: &str, config: AudioConfig) -> Result<Self, String> {
        validate_config(config)?;
        let name = CString::new(device).map_err(|_| "audio device name contains a NUL byte")?;
        let mut pcm = ptr::null_mut();
        let result = unsafe { snd_pcm_open(&mut pcm, name.as_ptr(), 0, SND_PCM_NONBLOCK) };
        if result < 0 {
            return Err(alsa_error("opening audio device", result));
        }

        let result = unsafe {
            snd_pcm_set_params(
                pcm as *mut SndPcm,
                2,
                3,
                u32::from(config.channels),
                config.sample_rate,
                1,
                config.latency_micros,
            )
        };
        if result < 0 {
            unsafe {
                snd_pcm_close(pcm);
            }
            return Err(alsa_error("configuring audio device", result));
        }
        Ok(Self { pcm, config })
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    /// Attempts to write interleaved samples without waiting for device capacity
    /// Returns the number of frames accepted, which may be less than requested
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<usize, String> {
        let channels = usize::from(self.config.channels);
        if !samples.len().is_multiple_of(channels) {
            return Err(format!(
                "sample count {} is not divisible by channel count {channels}",
                samples.len()
            ));
        }
        if samples.is_empty() {
            return Ok(0);
        }

        let pcm_samples: Vec<i16> = samples.iter().copied().map(float_to_pcm).collect();
        let frame_count = samples.len() / channels;
        write_available_frames(
            frame_count,
            |frames_written, remaining| {
                let samples_offset = frames_written * channels;
                unsafe {
                    snd_pcm_writei(
                        self.pcm,
                        pcm_samples[samples_offset..].as_ptr().cast(),
                        remaining as SndPcmUFrames,
                    )
                }
            },
            |error| unsafe { snd_pcm_recover(self.pcm, error, 1) },
        )
        .map_err(|error| alsa_error("recovering audio stream", error))
    }

    /// Waits for queued samples to finish and prepares the device for later writes.
    pub fn drain(&mut self) -> Result<(), String> {
        let result = unsafe { snd_pcm_drain(self.pcm) };
        if result < 0 {
            return Err(alsa_error("draining audio stream", result));
        }
        let result = unsafe { snd_pcm_prepare(self.pcm) };
        if result < 0 {
            return Err(alsa_error("preparing audio stream", result));
        }
        Ok(())
    }
}

fn write_available_frames(
    frame_count: usize,
    mut write: impl FnMut(usize, usize) -> SndPcmFrames,
    mut recover: impl FnMut(c_int) -> c_int,
) -> Result<usize, c_int> {
    let mut frames_written = 0;
    let mut recovered = false;
    while frames_written < frame_count {
        let result = write(frames_written, frame_count - frames_written);
        if result > 0 {
            frames_written += result as usize;
            recovered = false;
            continue;
        }
        if result == -(libc::EAGAIN as SndPcmFrames) || result == 0 {
            return Ok(frames_written);
        }
        if recovered {
            return Err(result as c_int);
        }
        let result = recover(result as c_int);
        if result < 0 {
            return Err(result);
        }
        recovered = true;
    }
    Ok(frames_written)
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        unsafe {
            snd_pcm_drop(self.pcm);
            snd_pcm_close(self.pcm);
        }
    }
}

fn validate_config(config: AudioConfig) -> Result<(), String> {
    if !(8_000..=192_000).contains(&config.sample_rate) {
        return Err("audio sample rate must be between 8000 and 192000 Hz".into());
    }
    if !(1..=8).contains(&config.channels) {
        return Err("audio channel count must be between 1 and 8".into());
    }
    if !(1_000..=2_000_000).contains(&config.latency_micros) {
        return Err("audio latency must be between 1000 and 2000000 microseconds".into());
    }
    Ok(())
}

fn float_to_pcm(sample: f32) -> i16 {
    if !sample.is_finite() {
        return 0;
    }
    let scaled = sample.clamp(-1.0, 1.0) * if sample < 0.0 { 32_768.0 } else { 32_767.0 };
    scaled.round() as i16
}

fn alsa_error(operation: &str, code: c_int) -> String {
    let message = unsafe { CStr::from_ptr(snd_strerror(code)) }.to_string_lossy();
    format!("{operation}: {message} ({code})")
}

#[cfg(test)]
mod tests {
    use super::{AudioConfig, float_to_pcm, validate_config};

    #[test]
    fn validates_audio_configuration() {
        assert!(validate_config(AudioConfig::default()).is_ok());
        assert!(
            validate_config(AudioConfig {
                sample_rate: 0,
                ..AudioConfig::default()
            })
            .is_err()
        );
        assert!(
            validate_config(AudioConfig {
                channels: 0,
                ..AudioConfig::default()
            })
            .is_err()
        );
    }

    #[test]
    fn converts_float_samples_to_saturating_pcm() {
        assert_eq!(float_to_pcm(-1.0), i16::MIN);
        assert_eq!(float_to_pcm(1.0), i16::MAX);
        assert_eq!(float_to_pcm(f32::NAN), 0);
    }

    #[test]
    fn returns_partial_frames_when_alsa_would_block() {
        let mut results = [4, -(libc::EAGAIN as super::SndPcmFrames)].into_iter();
        let written = super::write_available_frames(
            10,
            |_, _| results.next().expect("expected two simulated writes"),
            |_| panic!("EAGAIN must not trigger stream recovery"),
        )
        .unwrap();
        assert_eq!(written, 4);
    }

    #[test]
    fn writes_and_drains_the_alsa_null_device() {
        let mut output = super::AudioOutput::open_device("null", AudioConfig::default())
            .expect("open ALSA null PCM");
        assert_eq!(output.write_samples(&[0.0; 512]).unwrap(), 256);
        output.drain().unwrap();
    }
}
