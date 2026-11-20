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

use crate::{Event, WindowBackend, WindowHandle};

impl WindowHandle {
    pub fn create(title: &str, width: u32, height: u32) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            let wayland_first = std::env::var_os("WAYLAND_DISPLAY").is_some();
            let backend = if wayland_first {
                match crate::linux::wayland::Window::create_window(title, width, height) {
                    Ok(window) => Ok(WindowBackend::LinuxWayland(window)),
                    Err(wayland_error) => {
                        crate::linux::x11::Window::create_window(title, width, height)
                            .map(WindowBackend::LinuxX11)
                            .map_err(|x11_error| {
                                format!("Wayland: {wayland_error}; X11: {x11_error}")
                            })
                    }
                }
            } else {
                match crate::linux::x11::Window::create_window(title, width, height) {
                    Ok(window) => Ok(WindowBackend::LinuxX11(window)),
                    Err(x11_error) => {
                        crate::linux::wayland::Window::create_window(title, width, height)
                            .map(WindowBackend::LinuxWayland)
                            .map_err(|wayland_error| {
                                format!("X11: {x11_error}; Wayland: {wayland_error}")
                            })
                    }
                }
            };
            return backend
                .map(|backend| Self { backend })
                .map_err(|error| format!("failed to create a Linux window ({error})"));
        }

        #[cfg(target_os = "windows")]
        {
            let width =
                i32::try_from(width).map_err(|_| "window width exceeds the Windows API limit")?;
            let height =
                i32::try_from(height).map_err(|_| "window height exceeds the Windows API limit")?;
            let backend = crate::windows::Window::create_window(title, width, height)?;
            return Ok(Self {
                backend: WindowBackend::Windows(backend),
            });
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = (title, width, height);
            Err("Window creation is not supported on this platform".into())
        }
    }

    pub fn apply_settings(
        &mut self,
        fullscreen: bool,
        resizable: bool,
        borderless: bool,
    ) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => window.apply_cfg(fullscreen, resizable, borderless),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => {
                window.apply_cfg(fullscreen, resizable, borderless)
            }
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.apply_cfg(fullscreen, resizable, borderless),
        }
    }

    pub fn set_window_state(
        &mut self,
        _was_minimized: bool,
        minimized: bool,
        maximized: bool,
    ) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        if _was_minimized
            && !minimized
            && !maximized
            && matches!(&self.backend, WindowBackend::LinuxWayland(_))
        {
            return Err("Wayland has no client request to restore a minimized window".into());
        }
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => window.set_window_state(minimized, maximized),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => window.set_window_state(minimized, maximized),
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.set_window_state(minimized, maximized),
        }
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => window.set_title(title),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => window.set_title(title),
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.set_title(title),
        }
    }

    pub fn set_surrounding_text(
        &mut self,
        text: &str,
        cursor: usize,
        anchor: usize,
    ) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(_) => {
                Err("surrounding-text updates require Wayland text-input-v3".into())
            }
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => {
                window.set_surrounding_text(text, cursor, anchor)
            }
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(_) => {
                let _ = (text, cursor, anchor);
                Err("surrounding-text updates require Wayland text-input-v3".into())
            }
        }
    }

    pub fn set_size(&mut self, width: u32, height: u32) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => window.set_size(width, height),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(_) => {
                Err("Wayland does not support client-requested window sizes".into())
            }
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.set_size(width, height),
        }
    }

    pub fn set_position(&mut self, x: i32, y: i32) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => window.set_position(x, y),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(_) => {
                Err("Wayland compositors do not permit client window positioning".into())
            }
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.set_position(x, y),
        }
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => {
                window.set_visible(visible);
                Ok(())
            }
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(_) if !visible => {
                Err("Wayland does not support hiding an xdg-toplevel".into())
            }
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(_) => Ok(()),
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => {
                window.set_visible(visible);
                Ok(())
            }
        }
    }

    pub fn request_focus(&mut self) -> Result<(), String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => {
                window.request_focus();
                Ok(())
            }
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(_) => {
                Err("Wayland does not allow clients to request keyboard focus".into())
            }
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.request_focus(),
        }
    }

    pub fn poll_events(&mut self) -> Result<Vec<Event>, String> {
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => Ok(window.poll_events()),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => window.poll_events(),
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => Ok(window.poll_events()),
        }
    }
}
