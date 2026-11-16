use std::collections::BTreeSet;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Conventional primary button. Native event conversion preserves backend codes instead.
    Left,
    /// Conventional secondary button. Native event conversion preserves backend codes instead.
    Right,
    /// Conventional middle button. Native event conversion preserves backend codes instead.
    Middle,
    /// Backend-native button identifier.
    Native(u32),
}

impl MouseButton {
    /// Returns the code used by `InputState` for this button.
    pub fn code(self) -> u32 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Middle => 3,
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

/// Opaque backend-supplied input; `kind` and `code` have no cross-platform interpretation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RawInput {
    /// Backend-specific input category.
    pub kind: u32,
    /// Backend-specific input code.
    pub code: u32,
}

/// Window and input events; key/button identifiers retain backend-native values.
#[derive(Copy, Clone, Debug, PartialEq)]
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
    /// Backend-provided wheel deltas; units are not normalized across platforms.
    Scroll {
        delta_x: f64,
        delta_y: f64,
    },
    TextInput {
        codepoint: u32,
    },
    Raw(RawInput),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputState {
    keys_down: BTreeSet<u32>,
    buttons_down: BTreeSet<u32>,
    pointer_position: (i32, i32),
    pointer_inside: bool,
    focused: bool,
    scroll_delta: (f64, f64),
    text_input: Vec<char>,
}

impl InputState {
    pub fn apply(&mut self, event: &Event) {
        match *event {
            Event::FocusChanged { focused } => {
                self.focused = focused;
                if !focused {
                    self.keys_down.clear();
                    self.buttons_down.clear();
                }
            }
            Event::Key(Key { code, pressed }) => {
                if pressed {
                    self.keys_down.insert(code);
                } else {
                    self.keys_down.remove(&code);
                }
            }
            Event::Mouse { button, pressed } => {
                let code = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 3,
                    MouseButton::Native(code) => code,
                };
                if pressed {
                    self.buttons_down.insert(code);
                } else {
                    self.buttons_down.remove(&code);
                }
            }
            Event::PointerMoved { x, y } => self.pointer_position = (x, y),
            Event::PointerEntered => self.pointer_inside = true,
            Event::PointerLeft => self.pointer_inside = false,
            Event::Scroll { delta_x, delta_y } => {
                self.scroll_delta.0 += delta_x;
                self.scroll_delta.1 += delta_y;
            }
            Event::TextInput { codepoint } => {
                if let Some(character) = char::from_u32(codepoint) {
                    self.text_input.push(character);
                }
            }
            Event::CloseRequested
            | Event::Resized { .. }
            | Event::WindowMoved { .. }
            | Event::Raw(_) => {}
        }
    }

    pub fn is_key_down(&self, code: u32) -> bool {
        self.keys_down.contains(&code)
    }

    pub fn is_button_down(&self, code: u32) -> bool {
        self.buttons_down.contains(&code)
    }

    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.is_button_down(button.code())
    }

    pub fn pointer_position(&self) -> (i32, i32) {
        self.pointer_position
    }

    pub fn pointer_inside(&self) -> bool {
        self.pointer_inside
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn scroll_delta(&self) -> (f64, f64) {
        self.scroll_delta
    }

    pub fn take_text_input(&mut self) -> Vec<char> {
        std::mem::take(&mut self.text_input)
    }

    pub fn begin_frame(&mut self) {
        self.scroll_delta = (0.0, 0.0);
        self.text_input.clear();
    }
}
