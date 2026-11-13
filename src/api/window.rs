use crate::nucleus;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_WINDOW_ID: AtomicU32 = AtomicU32::new(1);

pub enum WindowBackend {
    #[cfg(target_os = "linux")]
    LinuxX11(nucleus::linux::x11::Window),
    #[cfg(target_os = "linux")]
    LinuxWayland(nucleus::linux::wayland::Window),
    #[cfg(target_os = "windows")]
    Windows(nucleus::windows::Window),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowCfg {
    pub fullscreen: bool,
    pub resizable: bool,
    pub borderless: bool,
    pub focused: bool,
    pub visible: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub should_close: bool,
}

impl Default for WindowCfg {
    fn default() -> Self {
        Self {
            fullscreen: false,
            resizable: true,
            borderless: false,
            focused: false,
            visible: true,
            minimized: false,
            maximized: false,
            should_close: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowId(u32);

impl WindowId {
    pub fn get(self) -> u32 {
        self.0
    }
}

pub struct Window {
    pub title: String,
    pub backend: WindowBackend,
    pub size: (u32, u32),
    pub position: (u32, u32),
    pub framebuffer_size: (u32, u32),
    pub scale_factor: f32,
    pub cfg: WindowCfg,
    pub id: WindowId,
    pub cursor_position: (f64, f64),
    pub cursor_inside: bool,
}

impl Window {
    pub fn create_window(title: &str, width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("window dimensions must be positive".into());
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            #[cfg(target_os = "linux")]
            let backend = {
                let wayland_first = std::env::var_os("WAYLAND_DISPLAY").is_some();
                let backend = if wayland_first {
                    match nucleus::linux::wayland::Window::create_window(title, width, height) {
                        Ok(window) => Ok(WindowBackend::LinuxWayland(window)),
                        Err(wayland_error) => {
                            nucleus::linux::x11::Window::create_window(title, width, height)
                                .map(WindowBackend::LinuxX11)
                                .map_err(|x11_error| {
                                    format!("Wayland: {wayland_error}; X11: {x11_error}")
                                })
                        }
                    }
                } else {
                    match nucleus::linux::x11::Window::create_window(title, width, height) {
                        Ok(window) => Ok(WindowBackend::LinuxX11(window)),
                        Err(x11_error) => {
                            nucleus::linux::wayland::Window::create_window(title, width, height)
                                .map(WindowBackend::LinuxWayland)
                                .map_err(|wayland_error| {
                                    format!("X11: {x11_error}; Wayland: {wayland_error}")
                                })
                        }
                    }
                };
                backend.map_err(|error| format!("failed to create a Linux window ({error})"))?
            };
            #[cfg(target_os = "windows")]
            let backend = WindowBackend::Windows(nucleus::windows::Window::create_window(
                title,
                i32::try_from(width).map_err(|_| "window width exceeds the Windows API limit")?,
                i32::try_from(height).map_err(|_| "window height exceeds the Windows API limit")?,
            )?);

            Ok(Self {
                title: title.to_owned(),
                backend,
                size: (width, height),
                position: (0, 0),
                framebuffer_size: (width, height),
                scale_factor: 1.0,
                cfg: WindowCfg::default(),
                id: WindowId(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed)),
                cursor_position: (0.0, 0.0),
                cursor_inside: false,
            })
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = (title, width, height);
            Err("Window creation is not supported on this platform".into())
        }
    }

    // Applies native settings and records them only after the backend accepts them.
    pub fn apply_cfg(&mut self, cfg: WindowCfg) -> Result<(), String> {
        if cfg.minimized && cfg.maximized {
            return Err("a window cannot be minimized and maximized at once".into());
        }
        if cfg.focused || !cfg.visible || cfg.minimized || cfg.maximized {
            return Err(
                "focus, visibility, minimize, and maximize settings are not supported yet".into(),
            );
        }
        match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => {
                window.apply_cfg(cfg.fullscreen, cfg.resizable, cfg.borderless)?;
            }
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => {
                window.apply_cfg(cfg.fullscreen, cfg.resizable, cfg.borderless)?;
            }
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => {
                window.apply_cfg(cfg.fullscreen, cfg.resizable, cfg.borderless)?;
            }
        }
        self.cfg = cfg;
        Ok(())
    }

    pub fn poll_events(&mut self) -> Result<Vec<nucleus::Event>, String> {
        let events = match &mut self.backend {
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxX11(window) => window.poll_events(),
            #[cfg(target_os = "linux")]
            WindowBackend::LinuxWayland(window) => window.poll_events()?,
            #[cfg(target_os = "windows")]
            WindowBackend::Windows(window) => window.poll_events(),
        };
        for event in &events {
            match event {
                nucleus::Event::CloseRequested => self.cfg.should_close = true,
                nucleus::Event::Resized { width, height } => {
                    self.size = (*width, *height);
                    self.framebuffer_size = (*width, *height);
                }
                _ => {}
            }
        }
        Ok(events)
    }
}
