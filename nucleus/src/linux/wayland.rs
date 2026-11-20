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

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

use crate::Event;

const NATIVE_EVENT_KEYSYM: u32 = 12;
const NATIVE_EVENT_TEXT_COMMIT: u32 = 13;
const NATIVE_EVENT_TEXT_PREEDIT: u32 = 14;
const NATIVE_EVENT_TEXT_DELETE_SURROUNDING: u32 = 15;
const NATIVE_EVENT_TEXT_COMMIT_CHUNK: u32 = 16;
const NATIVE_PREEDIT_TEXT_CAPACITY: usize = 512;
const MAX_SURROUNDING_TEXT_BYTES: usize = 4000;

fn prepare_surrounding_text(
    text: &str,
    cursor: usize,
    anchor: usize,
) -> Result<(CString, c_int, c_int), String> {
    if text.len() > MAX_SURROUNDING_TEXT_BYTES {
        return Err("Wayland surrounding text exceeds the 4000-byte protocol limit".into());
    }
    if !text.is_char_boundary(cursor) || !text.is_char_boundary(anchor) {
        return Err("Wayland surrounding-text offsets must be UTF-8 byte boundaries".into());
    }
    let text = CString::new(text).map_err(|_| "Wayland surrounding text contains NUL")?;
    let cursor = c_int::try_from(cursor).map_err(|_| "Wayland cursor offset exceeds i32")?;
    let anchor = c_int::try_from(anchor).map_err(|_| "Wayland anchor offset exceeds i32")?;
    Ok((text, cursor, anchor))
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NativeEvent {
    kind: u32,
    code: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pressed: u32,
    delta_x: f64,
    delta_y: f64,
    text: [u8; NATIVE_PREEDIT_TEXT_CAPACITY],
    cursor_begin: i32,
    cursor_end: i32,
    delete_before: u32,
    delete_after: u32,
}

impl Default for NativeEvent {
    fn default() -> Self {
        Self {
            kind: 0,
            code: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            pressed: 0,
            delta_x: 0.0,
            delta_y: 0.0,
            text: [0; NATIVE_PREEDIT_TEXT_CAPACITY],
            cursor_begin: 0,
            cursor_end: 0,
            delete_before: 0,
            delete_after: 0,
        }
    }
}

unsafe extern "C" {
    fn vagus_wayland_create(title: *const c_char, width: i32, height: i32) -> *mut c_void;
    fn vagus_wayland_poll(window: *mut c_void, events: *mut NativeEvent, capacity: i32) -> i32;
    fn vagus_wayland_display(window: *mut c_void) -> *mut c_void;
    fn vagus_wayland_surface(window: *mut c_void) -> *mut c_void;
    fn vagus_wayland_set_title(window: *mut c_void, title: *const c_char) -> c_int;
    fn vagus_wayland_set_surrounding_text(
        window: *mut c_void,
        text: *const c_char,
        cursor: c_int,
        anchor: c_int,
    ) -> c_int;
    fn vagus_wayland_apply_cfg(
        window: *mut c_void,
        fullscreen: c_int,
        resizable: c_int,
        borderless: c_int,
    ) -> c_int;
    fn vagus_wayland_set_window_state(
        window: *mut c_void,
        minimized: c_int,
        maximized: c_int,
    ) -> c_int;
    fn vagus_wayland_destroy(window: *mut c_void);
}

#[link(name = "xkbcommon")]
unsafe extern "C" {
    fn xkb_context_new(flags: c_int) -> *mut c_void;
    fn xkb_context_unref(context: *mut c_void);
    fn xkb_compose_table_new_from_locale(
        context: *mut c_void,
        locale: *const c_char,
        flags: c_int,
    ) -> *mut c_void;
    fn xkb_compose_table_unref(table: *mut c_void);
    fn xkb_compose_state_new(table: *mut c_void, flags: c_int) -> *mut c_void;
    fn xkb_compose_state_unref(state: *mut c_void);
    fn xkb_compose_state_feed(state: *mut c_void, keysym: u32) -> c_int;
    fn xkb_compose_state_get_status(state: *mut c_void) -> c_int;
    fn xkb_compose_state_reset(state: *mut c_void);
    fn xkb_compose_state_get_utf8(state: *mut c_void, buffer: *mut c_char, size: usize) -> c_int;
    fn xkb_keysym_to_utf32(keysym: u32) -> u32;
}

struct TextInput {
    context: *mut c_void,
    table: *mut c_void,
    state: *mut c_void,
}

impl TextInput {
    fn new() -> Self {
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
            .into_iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.is_empty())
            .unwrap_or_else(|| "C".into());
        Self::with_locale(&locale)
    }

    fn with_locale(locale: &str) -> Self {
        let Ok(locale) = CString::new(locale) else {
            return Self::empty();
        };
        let context = unsafe { xkb_context_new(0) };
        if context.is_null() {
            return Self::empty();
        }
        let table = unsafe { xkb_compose_table_new_from_locale(context, locale.as_ptr(), 0) };
        let state = if table.is_null() {
            ptr::null_mut()
        } else {
            unsafe { xkb_compose_state_new(table, 0) }
        };
        Self {
            context,
            table,
            state,
        }
    }

    fn empty() -> Self {
        Self {
            context: ptr::null_mut(),
            table: ptr::null_mut(),
            state: ptr::null_mut(),
        }
    }

    fn feed_keysym(&mut self, keysym: u32) -> Vec<u32> {
        if self.state.is_null() {
            return Self::direct_text(keysym);
        }
        unsafe { xkb_compose_state_feed(self.state, keysym) };
        match unsafe { xkb_compose_state_get_status(self.state) } {
            1 => Vec::new(),
            2 => self.composed_text(),
            3 => {
                unsafe { xkb_compose_state_reset(self.state) };
                Self::direct_text(keysym)
            }
            _ => Self::direct_text(keysym),
        }
    }

    fn composed_text(&mut self) -> Vec<u32> {
        let required = unsafe { xkb_compose_state_get_utf8(self.state, ptr::null_mut(), 0) };
        if required <= 0 || required > 4096 {
            unsafe { xkb_compose_state_reset(self.state) };
            return Vec::new();
        }
        let mut buffer = vec![0; required as usize + 1];
        let written = unsafe {
            xkb_compose_state_get_utf8(self.state, buffer.as_mut_ptr().cast(), buffer.len())
        };
        unsafe { xkb_compose_state_reset(self.state) };
        if written < 0 || written > required {
            return Vec::new();
        }
        std::str::from_utf8(&buffer[..written as usize])
            .map(|text| text.chars().map(u32::from).collect())
            .unwrap_or_default()
    }

    fn reset(&mut self) {
        if !self.state.is_null() {
            unsafe { xkb_compose_state_reset(self.state) };
        }
    }

    fn direct_text(keysym: u32) -> Vec<u32> {
        let codepoint = unsafe { xkb_keysym_to_utf32(keysym) };
        char::from_u32(codepoint)
            .map(|character| vec![character as u32])
            .unwrap_or_default()
    }
}

impl Drop for TextInput {
    fn drop(&mut self) {
        unsafe {
            if !self.state.is_null() {
                xkb_compose_state_unref(self.state);
            }
            if !self.table.is_null() {
                xkb_compose_table_unref(self.table);
            }
            if !self.context.is_null() {
                xkb_context_unref(self.context);
            }
        }
    }
}

pub struct Window {
    native: *mut c_void,
    text_input: TextInput,
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
        Ok(Self {
            native,
            text_input: TextInput::new(),
        })
    }

    pub fn poll_events(&mut self) -> Result<Vec<Event>, String> {
        let mut raw = [NativeEvent::default(); 512];
        let count =
            unsafe { vagus_wayland_poll(self.native, raw.as_mut_ptr(), raw.len() as c_int) };
        if count < 0 {
            return Err("Wayland event polling failed".into());
        }
        Ok(convert_native_events(
            &raw[..count as usize],
            &mut self.text_input,
        ))
    }

    pub fn display_ptr(&self) -> *mut c_void {
        unsafe { vagus_wayland_display(self.native) }
    }

    pub fn surface_ptr(&self) -> *mut c_void {
        unsafe { vagus_wayland_surface(self.native) }
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), String> {
        let title = CString::new(title).map_err(|_| "window title contains NUL")?;
        match unsafe { vagus_wayland_set_title(self.native, title.as_ptr()) } {
            0 => Ok(()),
            _ => Err("Wayland failed to queue the title change".into()),
        }
    }

    pub fn set_surrounding_text(
        &mut self,
        text: &str,
        cursor: usize,
        anchor: usize,
    ) -> Result<(), String> {
        let (text, cursor, anchor) = prepare_surrounding_text(text, cursor, anchor)?;
        match unsafe {
            vagus_wayland_set_surrounding_text(self.native, text.as_ptr(), cursor, anchor)
        } {
            0 => Ok(()),
            -1 => Err("Wayland text-input-v3 is unavailable for this window".into()),
            _ => Err("Wayland failed to queue surrounding text".into()),
        }
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

    pub fn set_window_state(&mut self, minimized: bool, maximized: bool) -> Result<(), String> {
        match unsafe {
            vagus_wayland_set_window_state(self.native, minimized as c_int, maximized as c_int)
        } {
            0 => Ok(()),
            -1 => Err("a window cannot be minimized and maximized simultaneously".into()),
            _ => Err("Wayland failed to queue the window state request".into()),
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

fn convert_native_event(event: &NativeEvent) -> Option<Event> {
    match event.kind {
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
            button: normalize_button_code(event.code),
            pressed: event.pressed != 0,
        }),
        5 => Some(Event::PointerEntered),
        6 => Some(Event::PointerLeft),
        7 => Some(Event::SurfaceScroll {
            delta_x: event.delta_x,
            delta_y: -event.delta_y,
        }),
        10 => Some(Event::Scroll {
            delta_x: normalize_value120_x(event.delta_x),
            delta_y: normalize_value120_y(event.delta_y),
        }),
        11 => Some(Event::Scroll {
            delta_x: normalize_discrete_scroll_x(event.delta_x),
            delta_y: normalize_discrete_scroll_y(event.delta_y),
        }),
        8 => Some(Event::Focused {
            focused: event.pressed != 0,
        }),
        9 => Some(Event::TextInput {
            codepoint: event.code,
        }),
        NATIVE_EVENT_TEXT_COMMIT => Some(Event::TextInput {
            codepoint: event.code,
        }),
        NATIVE_EVENT_TEXT_PREEDIT => {
            let text = CStr::from_bytes_until_nul(&event.text)
                .ok()?
                .to_str()
                .ok()?
                .to_owned();
            Some(Event::TextPreedit {
                text,
                cursor_begin: event.cursor_begin,
                cursor_end: event.cursor_end,
            })
        }
        NATIVE_EVENT_TEXT_DELETE_SURROUNDING => Some(Event::TextDeleteSurrounding {
            before_bytes: event.delete_before,
            after_bytes: event.delete_after,
        }),
        _ => None,
    }
}

fn convert_native_events(events: &[NativeEvent], text_input: &mut TextInput) -> Vec<Event> {
    let mut converted = Vec::with_capacity(events.len());
    for event in events {
        if event.kind == NATIVE_EVENT_TEXT_COMMIT_CHUNK {
            if let Ok(text) = CStr::from_bytes_until_nul(&event.text) {
                if let Ok(text) = text.to_str() {
                    converted.extend(text.chars().map(|character| Event::TextInput {
                        codepoint: character as u32,
                    }));
                }
            }
            continue;
        }
        if event.kind == NATIVE_EVENT_KEYSYM {
            converted.extend(
                text_input
                    .feed_keysym(event.code)
                    .into_iter()
                    .map(|codepoint| Event::TextInput { codepoint }),
            );
            continue;
        }
        if event.kind == 8 && event.pressed == 0 {
            text_input.reset();
        }
        if let Some(event) = convert_native_event(event) {
            converted.push(event);
        }
    }
    converted
}

fn normalize_button_code(code: u32) -> u32 {
    match code {
        0x110 => 1,
        0x111 => 2,
        0x112 => 3,
        0x113 => 4,
        0x114 => 5,
        other => other,
    }
}

fn normalize_value120_x(value: f64) -> f64 {
    value / 120.0
}

// Wayland uses surface coordinates (x right, y down); match X11/Windows (y up).
fn normalize_value120_y(value: f64) -> f64 {
    -value / 120.0
}

fn normalize_discrete_scroll_x(value: f64) -> f64 {
    value
}

fn normalize_discrete_scroll_y(value: f64) -> f64 {
    -value
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SURROUNDING_TEXT_BYTES, NativeEvent, TextInput, convert_native_event,
        normalize_button_code, prepare_surrounding_text,
    };
    use crate::Event;

    #[test]
    fn validates_surrounding_text_utf8_offsets_and_protocol_limit() {
        let (text, cursor, anchor) = prepare_surrounding_text("AéB", 3, 1).unwrap();
        assert_eq!(text.to_bytes(), "AéB".as_bytes());
        assert_eq!((cursor, anchor), (3, 1));
        assert!(
            prepare_surrounding_text(&"x".repeat(MAX_SURROUNDING_TEXT_BYTES), 4000, 4000).is_ok()
        );

        assert!(prepare_surrounding_text("AéB", 2, 1).is_err());
        assert!(prepare_surrounding_text("AéB", 5, 1).is_err());
        assert!(prepare_surrounding_text("A\0B", 1, 1).is_err());
        assert!(
            prepare_surrounding_text(&"x".repeat(MAX_SURROUNDING_TEXT_BYTES + 1), 0, 0).is_err()
        );
    }

    #[test]
    fn converts_wayland_pointer_focus_scroll_and_text_events() {
        assert_eq!(normalize_button_code(0x110), 1);
        assert_eq!(normalize_button_code(0x111), 2);
        assert_eq!(normalize_button_code(0x112), 3);
        assert_eq!(normalize_button_code(0x113), 4);
        assert_eq!(normalize_button_code(0x114), 5);
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 5,
                ..NativeEvent::default()
            }),
            Some(Event::PointerEntered)
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 7,
                delta_x: 180.0,
                delta_y: 240.0,
                ..NativeEvent::default()
            }),
            Some(Event::SurfaceScroll {
                delta_x: 180.0,
                delta_y: -240.0,
            })
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 8,
                pressed: 1,
                ..NativeEvent::default()
            }),
            Some(Event::Focused { focused: true })
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 9,
                code: 0x1f642,
                ..NativeEvent::default()
            }),
            Some(Event::TextInput { codepoint: 0x1f642 })
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: u32::MAX,
                ..NativeEvent::default()
            }),
            None
        );
    }

    #[test]
    fn converts_compositor_committed_ime_text_to_text_input_events() {
        let events = [
            NativeEvent {
                kind: 13,
                code: 0x6f22,
                ..NativeEvent::default()
            },
            NativeEvent {
                kind: 13,
                code: 0x1f642,
                ..NativeEvent::default()
            },
        ];
        let mut text_input = TextInput::new();

        assert_eq!(
            super::convert_native_events(&events, &mut text_input),
            vec![
                Event::TextInput { codepoint: 0x6f22 },
                Event::TextInput { codepoint: 0x1f642 },
            ]
        );
    }

    #[test]
    fn converts_committed_text_chunks_without_losing_unicode_scalars() {
        fn chunk(text: &str) -> NativeEvent {
            let mut event = NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_COMMIT_CHUNK,
                ..NativeEvent::default()
            };
            event.text[..text.len()].copy_from_slice(text.as_bytes());
            event
        }

        let events = [chunk("é🙂"), chunk("漢")];
        let mut text_input = TextInput::new();

        assert_eq!(
            super::convert_native_events(&events, &mut text_input),
            "é🙂漢"
                .chars()
                .map(|character| Event::TextInput {
                    codepoint: character as u32,
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn native_commit_encoder_preserves_text_longer_than_the_event_queue_limit() {
        unsafe extern "C" {
            fn vagus_wayland_encode_committed_utf8(
                text: *const std::ffi::c_char,
                output: *mut NativeEvent,
                capacity: usize,
            ) -> usize;
        }

        let text = "é".repeat(600);
        let c_text = std::ffi::CString::new(text.as_str()).unwrap();
        let mut events = [NativeEvent::default(); 512];
        let count = unsafe {
            vagus_wayland_encode_committed_utf8(c_text.as_ptr(), events.as_mut_ptr(), events.len())
        };
        assert!(count > 1);

        let mut text_input = TextInput::new();
        assert_eq!(
            super::convert_native_events(&events[..count], &mut text_input),
            text.chars()
                .map(|character| Event::TextInput {
                    codepoint: character as u32,
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn native_event_queue_preserves_burst_order_beyond_its_initial_capacity() {
        unsafe extern "C" {
            fn vagus_wayland_test_event_queue_burst(event_count: usize) -> usize;
        }

        assert_eq!(unsafe { vagus_wayland_test_event_queue_burst(2049) }, 2049);
    }

    #[test]
    fn native_commit_staging_preserves_more_than_512_chunks() {
        unsafe extern "C" {
            fn vagus_wayland_test_committed_text_burst(chunk_count: usize) -> usize;
        }

        assert_eq!(
            unsafe { vagus_wayland_test_committed_text_burst(513) },
            513
        );
    }

    #[test]
    fn converts_wayland_surrounding_text_deletion_and_replacement_in_protocol_order() {
        let events = [
            NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_PREEDIT,
                cursor_begin: -1,
                cursor_end: -1,
                ..NativeEvent::default()
            },
            NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_DELETE_SURROUNDING,
                delete_before: 3,
                delete_after: 2,
                ..NativeEvent::default()
            },
            NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_COMMIT,
                code: 'é' as u32,
                ..NativeEvent::default()
            },
            NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_PREEDIT,
                cursor_begin: 0,
                cursor_end: 0,
                ..NativeEvent::default()
            },
        ];
        let mut text_input = TextInput::new();

        assert_eq!(
            super::convert_native_events(&events, &mut text_input),
            vec![
                Event::TextPreedit {
                    text: String::new(),
                    cursor_begin: -1,
                    cursor_end: -1,
                },
                Event::TextDeleteSurrounding {
                    before_bytes: 3,
                    after_bytes: 2,
                },
                Event::TextInput {
                    codepoint: 'é' as u32,
                },
                Event::TextPreedit {
                    text: String::new(),
                    cursor_begin: 0,
                    cursor_end: 0,
                },
            ]
        );
    }

    #[test]
    fn converts_compositor_preedit_text_and_utf8_cursor_offsets() {
        let mut text = [0; super::NATIVE_PREEDIT_TEXT_CAPACITY];
        text[.."かな".len()].copy_from_slice("かな".as_bytes());

        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_PREEDIT,
                text,
                cursor_begin: 3,
                cursor_end: 6,
                ..NativeEvent::default()
            }),
            Some(Event::TextPreedit {
                text: "かな".into(),
                cursor_begin: 3,
                cursor_end: 6,
            })
        );
    }

    #[test]
    fn rejects_malformed_native_preedit_utf8() {
        let mut text = [0; super::NATIVE_PREEDIT_TEXT_CAPACITY];
        text[..2].copy_from_slice(&[0xff, 0]);

        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: super::NATIVE_EVENT_TEXT_PREEDIT,
                text,
                ..NativeEvent::default()
            }),
            None
        );
    }

    #[test]
    fn native_preedit_truncation_preserves_utf8_and_invalidates_cursor() {
        unsafe extern "C" {
            fn vagus_wayland_test_preedit_truncates_on_utf8_boundary() -> i32;
        }

        assert_eq!(
            unsafe { vagus_wayland_test_preedit_truncates_on_utf8_boundary() },
            1
        );
    }

    #[test]
    fn native_event_layout_matches_the_c_event_struct() {
        unsafe extern "C" {
            fn vagus_wayland_event_size() -> usize;
        }

        assert_eq!(std::mem::size_of::<NativeEvent>(), unsafe {
            vagus_wayland_event_size()
        });
    }

    #[test]
    fn converts_wayland_discrete_and_value120_scrolls_to_wheel_units() {
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 10,
                delta_y: 120.0,
                ..NativeEvent::default()
            }),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: -1.0,
            })
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 10,
                delta_x: 60.0,
                ..NativeEvent::default()
            }),
            Some(Event::Scroll {
                delta_x: 0.5,
                delta_y: 0.0,
            })
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 11,
                delta_x: 2.0,
                ..NativeEvent::default()
            }),
            Some(Event::Scroll {
                delta_x: 2.0,
                delta_y: 0.0,
            })
        );
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 11,
                delta_y: 2.0,
                ..NativeEvent::default()
            }),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: -2.0,
            })
        );
    }

    #[test]
    fn preserves_wayland_continuous_scroll_in_surface_units() {
        assert_eq!(
            convert_native_event(&NativeEvent {
                kind: 7,
                delta_x: 30.0,
                delta_y: 60.0,
                ..NativeEvent::default()
            }),
            Some(Event::SurfaceScroll {
                delta_x: 30.0,
                delta_y: -60.0,
            })
        );
    }

    #[test]
    fn raw_axis_scroll_is_suppressed_when_a_wheel_delta_exists() {
        unsafe extern "C" {
            fn vagus_wayland_should_emit_raw_axis_scroll(has_wheel_delta: i32) -> i32;
            fn vagus_wayland_should_defer_axis_scroll(pointer_version: u32) -> i32;
        }

        assert_eq!(unsafe { vagus_wayland_should_emit_raw_axis_scroll(0) }, 1);
        assert_eq!(unsafe { vagus_wayland_should_emit_raw_axis_scroll(1) }, 0);
        assert_eq!(unsafe { vagus_wayland_should_defer_axis_scroll(4) }, 0);
        assert_eq!(unsafe { vagus_wayland_should_defer_axis_scroll(5) }, 1);
        assert_eq!(unsafe { vagus_wayland_should_defer_axis_scroll(8) }, 1);
    }

    #[test]
    fn mixed_axis_scroll_detail_only_suppresses_raw_motion_on_that_axis() {
        unsafe extern "C" {
            fn vagus_wayland_filter_raw_axis_scroll(
                axis_x: f64,
                axis_x_valid: i32,
                axis_x_has_wheel_delta: i32,
                axis_y: f64,
                axis_y_valid: i32,
                axis_y_has_wheel_delta: i32,
                delta_x: *mut f64,
                delta_y: *mut f64,
            );
        }

        let mut delta_x = 0.0;
        let mut delta_y = 0.0;
        unsafe {
            vagus_wayland_filter_raw_axis_scroll(2.0, 1, 1, -3.5, 1, 0, &mut delta_x, &mut delta_y);
        }
        assert_eq!((delta_x, delta_y), (0.0, -3.5));

        unsafe {
            vagus_wayland_filter_raw_axis_scroll(1.25, 1, 0, 4.0, 1, 1, &mut delta_x, &mut delta_y);
        }
        assert_eq!((delta_x, delta_y), (1.25, 0.0));
    }

    #[test]
    fn utf8_decoder_rejects_truncated_sequences_within_the_available_slice() {
        unsafe extern "C" {
            fn vagus_wayland_decode_utf8_scalar(
                bytes: *const u8,
                available: usize,
                codepoint: *mut u32,
                consumed: *mut usize,
            ) -> i32;
        }

        let valid = [0xc3, 0xa9];
        let mut codepoint = 0;
        let mut consumed = 0;
        assert_eq!(
            unsafe {
                vagus_wayland_decode_utf8_scalar(
                    valid.as_ptr(),
                    valid.len(),
                    &mut codepoint,
                    &mut consumed,
                )
            },
            1
        );
        assert_eq!((codepoint, consumed), (0xe9, 2));

        let truncated = [0xe2, 0x82];
        assert_eq!(
            unsafe {
                vagus_wayland_decode_utf8_scalar(
                    truncated.as_ptr(),
                    truncated.len(),
                    &mut codepoint,
                    &mut consumed,
                )
            },
            0
        );
        assert_eq!(consumed, 1);
    }

    #[test]
    fn utf8_decoder_resumes_after_malformed_bytes_without_losing_valid_text() {
        unsafe extern "C" {
            fn vagus_wayland_decode_utf8_scalar(
                bytes: *const u8,
                available: usize,
                codepoint: *mut u32,
                consumed: *mut usize,
            ) -> i32;
        }

        let input = [0xf0, b'(', 0x8c, b'(', b'A'];
        let mut decoded = Vec::new();
        let mut offset = 0;
        while offset < input.len() {
            let mut codepoint = 0;
            let mut consumed = 0;
            let valid = unsafe {
                vagus_wayland_decode_utf8_scalar(
                    input[offset..].as_ptr(),
                    input.len() - offset,
                    &mut codepoint,
                    &mut consumed,
                )
            };
            assert!(consumed > 0);
            if valid != 0 {
                decoded.push(codepoint);
            }
            offset += consumed;
        }

        assert_eq!(decoded, vec![b'(' as u32, b'(' as u32, b'A' as u32]);
    }

    #[test]
    fn composes_dead_acute_with_the_following_letter() {
        let mut text_input = TextInput::new();

        assert!(text_input.feed_keysym(0xfe51).is_empty());
        assert_eq!(text_input.feed_keysym(b'e' as u32), vec![0x00e9]);
    }

    #[test]
    fn focus_loss_cancels_pending_wayland_compose_sequence() {
        let mut text_input = TextInput::new();
        let events = [
            NativeEvent {
                kind: 12,
                code: 0xfe51,
                ..NativeEvent::default()
            },
            NativeEvent {
                kind: 8,
                pressed: 0,
                ..NativeEvent::default()
            },
            NativeEvent {
                kind: 12,
                code: b'e' as u32,
                ..NativeEvent::default()
            },
        ];

        assert_eq!(
            super::convert_native_events(&events, &mut text_input),
            vec![
                Event::Focused { focused: false },
                Event::TextInput {
                    codepoint: b'e' as u32,
                },
            ]
        );
    }
}
