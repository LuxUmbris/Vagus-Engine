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

#[test]
fn public_audio_config_validation_is_consistent_across_backends() {
    use vagus::audio::AudioConfig;

    assert!(AudioConfig::default().validate().is_ok());
    assert!(
        AudioConfig {
            sample_rate: 7_999,
            ..AudioConfig::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        AudioConfig {
            channels: 3,
            ..AudioConfig::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        AudioConfig {
            latency_micros: 999,
            ..AudioConfig::default()
        }
        .validate()
        .is_err()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn public_audio_api_opens_writes_and_drains_through_the_api_wrapper() {
    use vagus::audio::{AudioConfig, AudioOutput};

    let config = AudioConfig::default();
    let mut output = AudioOutput::open_device("null", config).expect("open ALSA null device");
    assert_eq!(output.config(), config);
    assert!(output.write_samples(&[0.25]).is_err());
    let accepted = output
        .write_samples(&vec![0.0; 480 * usize::from(config.channels)])
        .expect("write interleaved frames");
    assert!(accepted <= 480);
    output.drain().expect("drain queued frames");
}
