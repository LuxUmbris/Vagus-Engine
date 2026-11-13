#[cfg(target_os = "linux")]
pub mod audio;
#[cfg(target_os = "windows")]
#[path = "audio_windows.rs"]
pub mod audio;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    CloseRequested,
    Resized { width: u32, height: u32 },
    Key { code: u32, pressed: bool },
    PointerMoved { x: i32, y: i32 },
    PointerButton { button: u32, pressed: bool },
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;
