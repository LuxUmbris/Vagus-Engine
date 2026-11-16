pub mod audio;
mod shader;
pub use shader::{ShaderDevice, ShaderModule, validate_spirv_words};
mod texture;
pub use texture::{GpuTexture, TextureDevice, validate_rgba8_texture_data};
pub mod rtx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    CloseRequested,
    Resized { width: u32, height: u32 },
    Moved { x: i32, y: i32 },
    Focused { focused: bool },
    Key { code: u32, pressed: bool },
    PointerMoved { x: i32, y: i32 },
    PointerButton { button: u32, pressed: bool },
    PointerEntered,
    PointerLeft,
    Scroll { delta_x: f64, delta_y: f64 },
    TextInput { codepoint: u32 },
    RawInput { kind: u32, code: u32 },
}

/// Opaque native-window capability consumed by the platform-neutral renderer.
pub struct WindowHandle {
    backend: WindowBackend,
}

enum WindowBackend {
    #[cfg(target_os = "linux")]
    LinuxX11(linux::x11::Window),
    #[cfg(target_os = "linux")]
    LinuxWayland(linux::wayland::Window),
    #[cfg(target_os = "windows")]
    Windows(windows::Window),
}

#[cfg(target_os = "linux")]
mod linux;
mod native_window;
mod vulkan;
#[cfg(target_os = "windows")]
mod windows;

pub use vulkan::Renderer;
