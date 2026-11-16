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

use crate::essential::window::InputState;
use nucleus::WindowHandle;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_WINDOW_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Requested native window properties. Window-state flags represent requests, not confirmed state.
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
    pub(crate) backend: WindowHandle,
    pub size: (u32, u32),
    pub position: (i32, i32),
    pub framebuffer_size: (u32, u32),
    pub scale_factor: f32,
    pub cfg: WindowCfg,
    pub id: WindowId,
    pub cursor_position: (f64, f64),
    pub cursor_inside: bool,
    pub input: InputState,
}

impl Window {
    pub fn create_window(title: &str, width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("window dimensions must be positive".into());
        }
        let backend = WindowHandle::create(title, width, height)?;
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
            input: InputState::default(),
        })
    }

    /// Applies native settings, returning an error for unsupported requests.
    pub fn apply_cfg(&mut self, cfg: WindowCfg) -> Result<(), String> {
        if cfg.minimized && cfg.maximized {
            return Err("a window cannot be minimized and maximized simultaneously".into());
        }
        self.backend
            .apply_settings(cfg.fullscreen, cfg.resizable, cfg.borderless)?;
        let state_changed =
            cfg.minimized != self.cfg.minimized || cfg.maximized != self.cfg.maximized;
        if state_changed {
            self.backend
                .set_window_state(self.cfg.minimized, cfg.minimized, cfg.maximized)?;
        }
        if state_changed && !cfg.visible {
            self.set_visible(false)?;
        }
        if cfg.visible != self.cfg.visible {
            self.set_visible(cfg.visible)?;
        }
        if cfg.focused && !self.cfg.focused {
            self.request_focus()?;
        }
        self.cfg = cfg;
        Ok(())
    }

    /// Requests minimization through the native backend.
    pub fn minimize(&mut self) -> Result<(), String> {
        let mut cfg = self.cfg;
        cfg.minimized = true;
        cfg.maximized = false;
        self.apply_cfg(cfg)
    }

    /// Requests maximization through the native backend.
    pub fn maximize(&mut self) -> Result<(), String> {
        let mut cfg = self.cfg;
        cfg.minimized = false;
        cfg.maximized = true;
        self.apply_cfg(cfg)
    }

    /// Requests restoration to a normal window state.
    pub fn restore(&mut self) -> Result<(), String> {
        let mut cfg = self.cfg;
        cfg.minimized = false;
        cfg.maximized = false;
        self.apply_cfg(cfg)
    }

    /// Returns the stable identifier assigned to this API window.
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Returns false after a close event or an explicit close request.
    pub fn is_open(&self) -> bool {
        !self.cfg.should_close
    }

    /// Marks the window as ready to close; the owner should stop rendering and drop it.
    pub fn request_close(&mut self) {
        self.cfg.should_close = true;
    }

    pub fn input_state(&self) -> &InputState {
        &self.input
    }

    pub fn input_state_mut(&mut self) -> &mut InputState {
        &mut self.input
    }

    /// Changes the native window title.
    pub fn set_title(&mut self, title: &str) -> Result<(), String> {
        if title.contains('\0') {
            return Err("window title contains NUL".into());
        }
        self.backend.set_title(title)?;
        self.title.clear();
        self.title.push_str(title);
        Ok(())
    }

    /// Requests a native resize; cached dimensions update when the backend reports it.
    pub fn set_size(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err("window dimensions must be positive".into());
        }
        self.backend.set_size(width, height)
    }

    /// Requests a screen position.
    pub fn set_position(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.backend.set_position(x, y)
    }

    /// Shows or hides the window.
    pub fn set_visible(&mut self, visible: bool) -> Result<(), String> {
        self.backend.set_visible(visible)?;
        self.cfg.visible = visible;
        Ok(())
    }

    /// Requests keyboard focus; the operating system or compositor may deny it.
    pub fn request_focus(&mut self) -> Result<(), String> {
        self.backend.request_focus()
    }

    pub fn poll_events(&mut self) -> Result<Vec<super::event::Event>, String> {
        self.input.begin_frame();
        let events = self.backend.poll_events()?;
        let events: Vec<super::event::Event> =
            events.into_iter().map(super::event::from_native).collect();
        for event in &events {
            self.input.apply(event);
            match event {
                super::event::Event::CloseRequested => self.cfg.should_close = true,
                super::event::Event::Resized { width, height } => {
                    self.size = (*width, *height);
                    self.framebuffer_size = (*width, *height);
                }
                super::event::Event::WindowMoved { x, y } => self.position = (*x, *y),
                super::event::Event::FocusChanged { focused } => self.cfg.focused = *focused,
                super::event::Event::PointerMoved { x, y } => {
                    self.cursor_position = (*x as f64, *y as f64);
                }
                super::event::Event::PointerEntered => self.cursor_inside = true,
                super::event::Event::PointerLeft => self.cursor_inside = false,
                _ => {}
            }
        }
        Ok(events)
    }
}
