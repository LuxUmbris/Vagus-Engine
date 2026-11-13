use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr;

use crate::nucleus::Event;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NativeEvent {
    kind: u32,
    code: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pressed: u32,
}

unsafe extern "C" {
    fn vagus_wayland_create(title: *const c_char, width: i32, height: i32) -> *mut c_void;
    fn vagus_wayland_poll(window: *mut c_void, events: *mut NativeEvent, capacity: i32) -> i32;
    fn vagus_wayland_display(window: *mut c_void) -> *mut c_void;
    fn vagus_wayland_surface(window: *mut c_void) -> *mut c_void;
    fn vagus_wayland_apply_cfg(
        window: *mut c_void,
        fullscreen: c_int,
        resizable: c_int,
        borderless: c_int,
    ) -> c_int;
    fn vagus_wayland_destroy(window: *mut c_void);
}

pub struct Window {
    native: *mut c_void,
}

impl Window {
    pub fn create_window(title: &str, width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err("window dimensions must be positive and fit in i32".into());
        }
        let title = CString::new(title).map_err(|_| "window title contains NUL")?;
        let native = unsafe { vagus_wayland_create(title.as_ptr(), width as i32, height as i32) };
        if native.is_null() {
            return Err(
                "Wayland window creation failed (check compositor and xdg-shell support)".into(),
            );
        }
        Ok(Self { native })
    }

    pub fn poll_events(&mut self) -> Result<Vec<Event>, String> {
        let mut raw = [NativeEvent::default(); 512];
        let count =
            unsafe { vagus_wayland_poll(self.native, raw.as_mut_ptr(), raw.len() as c_int) };
        if count < 0 {
            return Err("Wayland event polling failed".into());
        }
        Ok(raw[..count as usize]
            .iter()
            .filter_map(|event| match event.kind {
                0 => Some(Event::CloseRequested),
                1 => Some(Event::Resized {
                    width: event.width,
                    height: event.height,
                }),
                2 => Some(Event::Key {
                    code: event.code,
                    pressed: event.pressed != 0,
                }),
                3 => Some(Event::PointerMoved {
                    x: event.x,
                    y: event.y,
                }),
                4 => Some(Event::PointerButton {
                    button: event.code,
                    pressed: event.pressed != 0,
                }),
                _ => None,
            })
            .collect())
    }

    pub fn display_ptr(&self) -> *mut c_void {
        unsafe { vagus_wayland_display(self.native) }
    }

    pub fn surface_ptr(&self) -> *mut c_void {
        unsafe { vagus_wayland_surface(self.native) }
    }

    pub fn apply_cfg(
        &mut self,
        fullscreen: bool,
        resizable: bool,
        borderless: bool,
    ) -> Result<(), String> {
        match unsafe {
            vagus_wayland_apply_cfg(
                self.native,
                fullscreen as c_int,
                resizable as c_int,
                borderless as c_int,
            )
        } {
            0 => Ok(()),
            -1 => Err("Wayland compositor does not support borderless decorations".into()),
            _ => Err("Wayland failed to queue the window configuration".into()),
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.native.is_null() {
            unsafe { vagus_wayland_destroy(self.native) };
            self.native = ptr::null_mut();
        }
    }
}
