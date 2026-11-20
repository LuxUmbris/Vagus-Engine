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

use std::{collections::BTreeSet, io};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Primary pointer button, normalized across supported backends.
    Left,
    /// Secondary pointer button, normalized across supported backends.
    Right,
    /// Middle pointer button, normalized across supported backends.
    Middle,
    /// Back-navigation button, normalized across supported backends.
    Back,
    /// Forward-navigation button, normalized across supported backends.
    Forward,
    /// Backend-specific code for buttons other than the normalized five.
    Native(u32),
}

impl MouseButton {
    /// Returns the code used by `InputState` for this button.
    pub fn code(self) -> u32 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Middle => 3,
            Self::Back => 4,
            Self::Forward => 5,
            Self::Native(code) => code,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Key {
    /// Native key identifier; its meaning depends on the active backend.
    pub code: u32,
    pub pressed: bool,
}

/// Current IME composition text. Cursor/selection offsets are UTF-8 byte offsets;
/// negative values mean the input method did not specify a cursor. Wayland
/// text-input-v3 and X11 XIM callback styles can emit preedit events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPreedit {
    pub text: String,
    pub cursor_begin: i32,
    pub cursor_end: i32,
}

/// Ordered text-edit operations emitted by an input method within a frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    /// Delete UTF-8 bytes around the active cursor before applying later commits.
    DeleteSurrounding { before_bytes: u32, after_bytes: u32 },
    /// Insert one committed Unicode scalar at the active cursor.
    Commit(char),
}

impl TextInputEvent {
    /// Applies this edit to UTF-8 text and its byte-offset cursor.
    ///
    /// Deletion ranges must land on character boundaries. Invalid ranges leave both arguments
    /// unchanged; committed characters are inserted only after their required capacity is reserved.
    pub fn apply_to(&self, text: &mut String, cursor_byte_offset: &mut usize) -> io::Result<()> {
        if *cursor_byte_offset > text.len() || !text.is_char_boundary(*cursor_byte_offset) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "text input cursor is not a UTF-8 character boundary",
            ));
        }

        match *self {
            Self::DeleteSurrounding {
                before_bytes,
                after_bytes,
            } => {
                let before = usize::try_from(before_bytes).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "text deletion range overflow")
                })?;
                let after = usize::try_from(after_bytes).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "text deletion range overflow")
                })?;
                let start = cursor_byte_offset.checked_sub(before).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "text deletion starts before text",
                    )
                })?;
                let end = cursor_byte_offset
                    .checked_add(after)
                    .filter(|&end| end <= text.len())
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "text deletion ends after text")
                    })?;
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "text deletion range is not on UTF-8 character boundaries",
                    ));
                }

                text.replace_range(start..end, "");
                *cursor_byte_offset = start;
            }
            Self::Commit(character) => {
                let next_cursor = cursor_byte_offset
                    .checked_add(character.len_utf8())
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "text cursor overflow")
                    })?;
                text.try_reserve(character.len_utf8())
                    .map_err(|error| io::Error::other(error.to_string()))?;
                text.insert(*cursor_byte_offset, character);
                *cursor_byte_offset = next_cursor;
            }
        }
        Ok(())
    }
}

/// Opaque backend-supplied input; `kind` and `code` have no cross-platform interpretation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RawInput {
    /// Backend-specific input category.
    pub kind: u32,
    /// Backend-specific input code.
    pub code: u32,
}

/// Window and input events. Key identifiers and buttons beyond the first five remain
/// backend-native; the primary and auxiliary mouse buttons use standardized variants.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    CloseRequested,
    Resized {
        width: u32,
        height: u32,
    },
    WindowMoved {
        x: i32,
        y: i32,
    },
    FocusChanged {
        focused: bool,
    },
    Mouse {
        button: MouseButton,
        pressed: bool,
    },
    Key(Key),
    /// Pointer coordinates in the backend's client/surface coordinate space.
    PointerMoved {
        x: i32,
        y: i32,
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
    /// One committed Unicode scalar of text; distinct from physical key events.
    TextInput {
        codepoint: u32,
    },
    /// Replace the current native IME preedit; an empty string clears it.
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
    Raw(RawInput),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputState {
    keys_down: BTreeSet<u32>,
    keys_pressed: BTreeSet<u32>,
    keys_released: BTreeSet<u32>,
    buttons_down: BTreeSet<u32>,
    buttons_pressed: BTreeSet<u32>,
    buttons_released: BTreeSet<u32>,
    pointer_position: (i32, i32),
    pointer_inside: bool,
    pointer_entered: bool,
    pointer_left: bool,
    focused: bool,
    scroll_delta: (f64, f64),
    surface_scroll_delta: (f64, f64),
    text_input: Vec<char>,
    text_edits: Vec<TextInputEvent>,
    text_preedit: Option<TextPreedit>,
}

fn accumulate_finite_delta(current: f64, delta: f64) -> f64 {
    if !delta.is_finite() {
        return current;
    }

    let accumulated = current + delta;
    if accumulated.is_finite() {
        accumulated
    } else {
        current
    }
}

impl InputState {
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::FocusChanged { focused } => {
                self.focused = *focused;
                if !focused {
                    self.keys_released.extend(self.keys_down.iter().copied());
                    self.buttons_released
                        .extend(self.buttons_down.iter().copied());
                    self.keys_down.clear();
                    self.buttons_down.clear();
                    self.text_preedit = None;
                }
            }
            Event::Key(Key { code, pressed }) => {
                if *pressed {
                    if self.keys_down.insert(*code) {
                        self.keys_pressed.insert(*code);
                    }
                } else if self.keys_down.remove(code) {
                    self.keys_released.insert(*code);
                }
            }
            Event::Mouse { button, pressed } => {
                let code = button.code();
                if *pressed {
                    if self.buttons_down.insert(code) {
                        self.buttons_pressed.insert(code);
                    }
                } else if self.buttons_down.remove(&code) {
                    self.buttons_released.insert(code);
                }
            }
            Event::PointerMoved { x, y } => self.pointer_position = (*x, *y),
            Event::PointerEntered => {
                if !self.pointer_inside {
                    self.pointer_entered = true;
                    self.pointer_inside = true;
                }
            }
            Event::PointerLeft => {
                if self.pointer_inside {
                    self.pointer_left = true;
                    self.pointer_inside = false;
                }
            }
            Event::Scroll { delta_x, delta_y } => {
                self.scroll_delta.0 = accumulate_finite_delta(self.scroll_delta.0, *delta_x);
                self.scroll_delta.1 = accumulate_finite_delta(self.scroll_delta.1, *delta_y);
            }
            Event::SurfaceScroll { delta_x, delta_y } => {
                self.surface_scroll_delta.0 =
                    accumulate_finite_delta(self.surface_scroll_delta.0, *delta_x);
                self.surface_scroll_delta.1 =
                    accumulate_finite_delta(self.surface_scroll_delta.1, *delta_y);
            }
            Event::TextInput { codepoint } => {
                if let Some(character) = char::from_u32(*codepoint) {
                    self.text_input.push(character);
                    self.text_edits.push(TextInputEvent::Commit(character));
                }
            }
            Event::TextPreedit {
                text,
                cursor_begin,
                cursor_end,
            } => {
                self.text_preedit = if text.is_empty() {
                    None
                } else {
                    Some(TextPreedit {
                        text: text.clone(),
                        cursor_begin: *cursor_begin,
                        cursor_end: *cursor_end,
                    })
                };
            }
            Event::CloseRequested
            | Event::Resized { .. }
            | Event::WindowMoved { .. }
            | Event::Raw(_) => {}
            Event::TextDeleteSurrounding {
                before_bytes,
                after_bytes,
            } => self.text_edits.push(TextInputEvent::DeleteSurrounding {
                before_bytes: *before_bytes,
                after_bytes: *after_bytes,
            }),
        }
    }

    pub fn is_key_down(&self, code: u32) -> bool {
        self.keys_down.contains(&code)
    }

    pub fn was_key_pressed(&self, code: u32) -> bool {
        self.keys_pressed.contains(&code)
    }

    pub fn was_key_released(&self, code: u32) -> bool {
        self.keys_released.contains(&code)
    }

    pub fn is_button_down(&self, code: u32) -> bool {
        self.buttons_down.contains(&code)
    }

    pub fn was_button_pressed(&self, code: u32) -> bool {
        self.buttons_pressed.contains(&code)
    }

    pub fn was_button_released(&self, code: u32) -> bool {
        self.buttons_released.contains(&code)
    }

    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.is_button_down(button.code())
    }

    pub fn was_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.was_button_pressed(button.code())
    }

    pub fn was_mouse_button_released(&self, button: MouseButton) -> bool {
        self.was_button_released(button.code())
    }

    pub fn pointer_position(&self) -> (i32, i32) {
        self.pointer_position
    }

    /// Returns pointer coordinates as fractions of the client extent.
    ///
    /// `client_width` and `client_height` must use the same local units as the native pointer
    /// events. Coordinates are not clamped, so positions outside the client area remain outside
    /// the `0.0..=1.0` range. Zero-sized extents return `None`.
    pub fn pointer_position_normalized(
        &self,
        client_width: u32,
        client_height: u32,
    ) -> Option<(f64, f64)> {
        if client_width == 0 || client_height == 0 {
            return None;
        }

        Some((
            f64::from(self.pointer_position.0) / f64::from(client_width),
            f64::from(self.pointer_position.1) / f64::from(client_height),
        ))
    }

    pub fn pointer_inside(&self) -> bool {
        self.pointer_inside
    }

    pub fn was_pointer_entered(&self) -> bool {
        self.pointer_entered
    }

    pub fn was_pointer_left(&self) -> bool {
        self.pointer_left
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn scroll_delta(&self) -> (f64, f64) {
        self.scroll_delta
    }

    /// Continuous scroll in surface-coordinate units, separate from wheel detents.
    pub fn surface_scroll_delta(&self) -> (f64, f64) {
        self.surface_scroll_delta
    }

    /// Takes committed characters only; use `take_text_edits` to preserve IME edit ordering.
    pub fn take_text_input(&mut self) -> Vec<char> {
        std::mem::take(&mut self.text_input)
    }

    /// Takes the ordered IME delete/commit operations accumulated this frame.
    pub fn take_text_edits(&mut self) -> Vec<TextInputEvent> {
        std::mem::take(&mut self.text_edits)
    }

    pub fn text_preedit(&self) -> Option<&TextPreedit> {
        self.text_preedit.as_ref()
    }

    pub fn begin_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.pointer_entered = false;
        self.pointer_left = false;
        self.scroll_delta = (0.0, 0.0);
        self.surface_scroll_delta = (0.0, 0.0);
        self.text_input.clear();
        self.text_edits.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, InputState, Key};

    #[test]
    fn key_transitions_are_idempotent_and_frame_scoped() {
        let mut input = InputState::default();
        input.apply(&Event::Key(Key {
            code: 17,
            pressed: true,
        }));
        input.apply(&Event::Key(Key {
            code: 17,
            pressed: true,
        }));

        assert!(input.was_key_pressed(17));
        assert!(!input.was_key_released(17));
        assert!(input.is_key_down(17));

        input.apply(&Event::Key(Key {
            code: 17,
            pressed: false,
        }));
        assert!(input.was_key_pressed(17));
        assert!(input.was_key_released(17));
        assert!(!input.is_key_down(17));

        input.begin_frame();
        assert!(!input.was_key_pressed(17));
        assert!(!input.was_key_released(17));
    }
}
