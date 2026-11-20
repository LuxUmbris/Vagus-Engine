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

pub mod audio;
mod input_text;
mod shader;
pub use shader::{ShaderDevice, ShaderModule, validate_spirv_words};
mod texture;
pub use texture::{GpuTexture, TextureDevice, validate_rgba8_texture_data};
pub mod rtx;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    CloseRequested,
    Resized {
        width: u32,
        height: u32,
    },
    Moved {
        x: i32,
        y: i32,
    },
    Focused {
        focused: bool,
    },
    Key {
        code: u32,
        pressed: bool,
    },
    PointerMoved {
        x: i32,
        y: i32,
    },
    /// Pointer button code: 1=left, 2=right, 3=middle, 4=back, 5=forward; others are native.
    PointerButton {
        button: u32,
        pressed: bool,
    },
    PointerEntered,
    PointerLeft,
    /// Normalized wheel deltas; 1.0 is one detent. Positive X is right; positive Y is up.
    Scroll {
        delta_x: f64,
        delta_y: f64,
    },
    /// Continuous scroll in backend surface-coordinate units. Positive X is right; positive Y is up.
    SurfaceScroll {
        delta_x: f64,
        delta_y: f64,
    },
    TextInput {
        codepoint: u32,
    },
    TextPreedit {
        text: String,
        cursor_begin: i32,
        cursor_end: i32,
    },
    /// Delete UTF-8 bytes around the active cursor before applying committed text.
    TextDeleteSurrounding {
        before_bytes: u32,
        after_bytes: u32,
    },
    RawInput {
        kind: u32,
        code: u32,
    },
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn windows_scroll_event(wparam: usize, horizontal: bool) -> Event {
    let delta = f64::from((wparam >> 16) as u16 as i16) / 120.0;
    if horizontal {
        Event::Scroll {
            delta_x: delta,
            delta_y: 0.0,
        }
    } else {
        Event::Scroll {
            delta_x: 0.0,
            delta_y: delta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{windows_scroll_event, Event};

    #[test]
    fn windows_scroll_normalizes_signed_partial_detents_on_each_axis() {
        let wparam = |delta: i16| ((delta as u16 as usize) << 16) | 0x0005;
        assert_eq!(
            windows_scroll_event(wparam(60), false),
            Event::Scroll {
                delta_x: 0.0,
                delta_y: 0.5,
            }
        );
        assert_eq!(
            windows_scroll_event(wparam(-120), false),
            Event::Scroll {
                delta_x: 0.0,
                delta_y: -1.0,
            }
        );
        assert_eq!(
            windows_scroll_event(wparam(-30), true),
            Event::Scroll {
                delta_x: -0.25,
                delta_y: 0.0,
            }
        );
        assert_eq!(
            windows_scroll_event(wparam(120), true),
            Event::Scroll {
                delta_x: 1.0,
                delta_y: 0.0,
            }
        );
    }
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

pub use vulkan::{
    GpuCameraProjection, GpuInstanceTransform, GpuMeshVertex, GpuTextureQuad, GpuTriangle, Renderer,
};
