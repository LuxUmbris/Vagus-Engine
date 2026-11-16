#[cfg(target_os = "linux")]
pub use super::linux::audio::{AudioConfig, AudioOutput};

#[cfg(target_os = "windows")]
pub use super::windows::audio::{AudioConfig, AudioOutput};
