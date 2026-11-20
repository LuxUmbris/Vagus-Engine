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

pub use crate::essential::window::{
    Event, InputState, Key, MouseButton, RawInput, TextInputEvent, TextPreedit,
};

pub(super) fn from_native(event: nucleus::Event) -> Event {
    match event {
        nucleus::Event::CloseRequested => Event::CloseRequested,
        nucleus::Event::Resized { width, height } => Event::Resized { width, height },
        nucleus::Event::Moved { x, y } => Event::WindowMoved { x, y },
        nucleus::Event::Focused { focused } => Event::FocusChanged { focused },
        nucleus::Event::Key { code, pressed } => Event::Key(Key { code, pressed }),
        nucleus::Event::PointerMoved { x, y } => Event::PointerMoved { x, y },
        nucleus::Event::PointerButton { button, pressed } => Event::Mouse {
            button: match button {
                1 => MouseButton::Left,
                2 => MouseButton::Right,
                3 => MouseButton::Middle,
                4 => MouseButton::Back,
                5 => MouseButton::Forward,
                other => MouseButton::Native(other),
            },
            pressed,
        },
        nucleus::Event::PointerEntered => Event::PointerEntered,
        nucleus::Event::PointerLeft => Event::PointerLeft,
        nucleus::Event::Scroll { delta_x, delta_y } => Event::Scroll { delta_x, delta_y },
        nucleus::Event::SurfaceScroll { delta_x, delta_y } => {
            Event::SurfaceScroll { delta_x, delta_y }
        }
        nucleus::Event::TextInput { codepoint } => Event::TextInput { codepoint },
        nucleus::Event::TextPreedit {
            text,
            cursor_begin,
            cursor_end,
        } => Event::TextPreedit {
            text,
            cursor_begin,
            cursor_end,
        },
        nucleus::Event::TextDeleteSurrounding {
            before_bytes,
            after_bytes,
        } => Event::TextDeleteSurrounding {
            before_bytes,
            after_bytes,
        },
        nucleus::Event::RawInput { kind, code } => Event::Raw(RawInput { kind, code }),
    }
}

#[cfg(test)]
mod tests {
    use super::{from_native, Event, InputState, Key, MouseButton, RawInput};
    use nucleus;

    #[test]
    fn native_events_convert_without_losing_payloads() {
        assert_eq!(
            from_native(nucleus::Event::CloseRequested),
            Event::CloseRequested
        );
        assert_eq!(
            from_native(nucleus::Event::Resized {
                width: 1280,
                height: 720
            }),
            Event::Resized {
                width: 1280,
                height: 720
            },
        );
        assert_eq!(
            from_native(nucleus::Event::SurfaceScroll {
                delta_x: 24.0,
                delta_y: -12.0,
            }),
            Event::SurfaceScroll {
                delta_x: 24.0,
                delta_y: -12.0,
            },
        );
        assert_eq!(
            from_native(nucleus::Event::Key {
                code: 0x1234,
                pressed: true
            }),
            Event::Key(Key {
                code: 0x1234,
                pressed: true
            }),
        );
        assert_eq!(
            from_native(nucleus::Event::PointerButton {
                button: 0x5678,
                pressed: false
            }),
            Event::Mouse {
                button: MouseButton::Native(0x5678),
                pressed: false
            },
        );
        assert_eq!(
            from_native(nucleus::Event::RawInput {
                kind: 77,
                code: 0x1234
            }),
            Event::Raw(RawInput {
                kind: 77,
                code: 0x1234
            }),
        );
        assert_eq!(
            from_native(nucleus::Event::TextPreedit {
                text: "かな".into(),
                cursor_begin: 3,
                cursor_end: 6,
            }),
            Event::TextPreedit {
                text: "かな".into(),
                cursor_begin: 3,
                cursor_end: 6,
            },
        );
        assert_eq!(
            from_native(nucleus::Event::TextDeleteSurrounding {
                before_bytes: 3,
                after_bytes: 2,
            }),
            Event::TextDeleteSurrounding {
                before_bytes: 3,
                after_bytes: 2,
            },
        );
    }

    #[test]
    fn maps_standard_pointer_buttons_to_cross_platform_variants() {
        for (button, expected) in [
            (1, MouseButton::Left),
            (2, MouseButton::Right),
            (3, MouseButton::Middle),
        ] {
            assert_eq!(
                from_native(nucleus::Event::PointerButton {
                    button,
                    pressed: true,
                }),
                Event::Mouse {
                    button: expected,
                    pressed: true,
                },
            );
        }

        assert_eq!(
            from_native(nucleus::Event::PointerButton {
                button: 6,
                pressed: false,
            }),
            Event::Mouse {
                button: MouseButton::Native(6),
                pressed: false,
            },
        );
    }

    #[test]
    fn maps_auxiliary_pointer_buttons_to_back_and_forward() {
        assert_eq!(
            from_native(nucleus::Event::PointerButton {
                button: 4,
                pressed: true,
            }),
            Event::Mouse {
                button: MouseButton::Back,
                pressed: true,
            },
        );
        assert_eq!(
            from_native(nucleus::Event::PointerButton {
                button: 5,
                pressed: false,
            }),
            Event::Mouse {
                button: MouseButton::Forward,
                pressed: false,
            },
        );
    }

    #[test]
    fn converted_native_events_drive_focus_loss_release_transitions() {
        let mut input = InputState::default();
        for event in [
            nucleus::Event::Key {
                code: 17,
                pressed: true,
            },
            nucleus::Event::PointerButton {
                button: 4,
                pressed: true,
            },
        ] {
            input.apply(&from_native(event));
        }

        assert!(input.is_key_down(17));
        assert!(input.was_key_pressed(17));
        assert!(input.is_mouse_button_down(MouseButton::Back));
        assert!(input.was_mouse_button_pressed(MouseButton::Back));

        input.begin_frame();
        input.apply(&from_native(nucleus::Event::Focused { focused: false }));

        assert!(!input.is_key_down(17));
        assert!(input.was_key_released(17));
        assert!(!input.is_mouse_button_down(MouseButton::Back));
        assert!(input.was_mouse_button_released(MouseButton::Back));
    }
}
