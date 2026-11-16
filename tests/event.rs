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

use vagus::event::{Event, Key, MouseButton, RawInput};

#[test]
fn raw_input_preserves_unmapped_native_input() {
    assert_eq!(
        Event::Raw(RawInput {
            kind: 77,
            code: 0x1234
        }),
        Event::Raw(RawInput {
            kind: 77,
            code: 0x1234
        })
    );
}

#[test]
fn essential_input_state_tracks_keys_buttons_focus_and_pointer() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::Key(Key {
        code: 0x1234,
        pressed: true,
    }));
    input.apply(&Event::Mouse {
        button: MouseButton::Native(5),
        pressed: true,
    });
    input.apply(&Event::PointerMoved { x: -10, y: 20 });
    input.apply(&Event::PointerEntered);
    input.apply(&Event::FocusChanged { focused: true });

    assert!(input.is_key_down(0x1234));
    assert!(input.is_button_down(5));
    assert!(input.is_mouse_button_down(MouseButton::Native(5)));
    assert_eq!(input.pointer_position(), (-10, 20));
    assert!(input.pointer_inside());
    assert!(input.is_focused());

    input.apply(&Event::Key(Key {
        code: 0x1234,
        pressed: false,
    }));
    input.apply(&Event::Mouse {
        button: MouseButton::Native(5),
        pressed: false,
    });
    input.apply(&Event::PointerLeft);
    input.apply(&Event::FocusChanged { focused: false });
    assert!(!input.is_key_down(0x1234));
    assert!(!input.is_button_down(5));
    assert!(!input.pointer_inside());
    assert!(!input.is_focused());
}

#[test]
fn input_state_accumulates_frame_text_and_scroll_then_clears_them() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::Scroll {
        delta_x: 0.5,
        delta_y: -1.25,
    });
    input.apply(&Event::TextInput { codepoint: 0x1f642 });
    input.apply(&Event::TextInput { codepoint: 0xd800 });
    assert_eq!(input.scroll_delta(), (0.5, -1.25));
    assert_eq!(input.take_text_input(), vec!['🙂']);

    input.apply(&Event::Key(Key {
        code: 42,
        pressed: true,
    }));
    input.begin_frame();
    assert_eq!(input.scroll_delta(), (0.0, 0.0));
    assert!(input.take_text_input().is_empty());
    assert!(input.is_key_down(42));
}
