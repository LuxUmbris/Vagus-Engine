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
fn input_state_reports_mouse_pointer_and_focus_loss_transitions_per_frame() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::FocusChanged { focused: true });
    input.apply(&Event::Mouse {
        button: MouseButton::Left,
        pressed: true,
    });
    input.apply(&Event::Mouse {
        button: MouseButton::Left,
        pressed: true,
    });
    input.apply(&Event::Key(Key {
        code: 17,
        pressed: true,
    }));
    input.apply(&Event::PointerEntered);

    assert!(input.was_mouse_button_pressed(MouseButton::Left));
    assert!(input.was_key_pressed(17));
    assert!(input.was_pointer_entered());

    input.begin_frame();
    assert!(!input.was_mouse_button_pressed(MouseButton::Left));
    assert!(!input.was_key_pressed(17));
    assert!(!input.was_pointer_entered());
    assert!(input.is_mouse_button_down(MouseButton::Left));
    assert!(input.is_key_down(17));
    assert!(input.pointer_inside());

    input.apply(&Event::FocusChanged { focused: false });
    assert!(input.was_mouse_button_released(MouseButton::Left));
    assert!(input.was_key_released(17));
    assert!(!input.is_mouse_button_down(MouseButton::Left));
    assert!(!input.is_key_down(17));

    input.apply(&Event::PointerLeft);
    assert!(input.was_pointer_left());
    assert!(!input.pointer_inside());
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

#[test]
fn input_state_discards_non_finite_scroll_and_prevents_accumulation_overflow() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::Scroll {
        delta_x: f64::NAN,
        delta_y: 0.5,
    });
    input.apply(&Event::Scroll {
        delta_x: 0.25,
        delta_y: f64::INFINITY,
    });
    assert_eq!(input.scroll_delta(), (0.25, 0.5));

    input.apply(&Event::Scroll {
        delta_x: f64::MAX,
        delta_y: -f64::MAX,
    });
    let accumulated = input.scroll_delta();
    input.apply(&Event::Scroll {
        delta_x: f64::MAX,
        delta_y: -f64::MAX,
    });
    let (delta_x, delta_y) = input.scroll_delta();
    assert_eq!((delta_x, delta_y), accumulated);
    assert!(delta_x.is_finite());
    assert!(delta_y.is_finite());
}

#[test]
fn input_state_tracks_text_preedit_and_cursor_range_until_cleared() {
    use vagus::essential::window::{InputState, TextPreedit};

    let mut input = InputState::default();
    input.apply(&Event::TextPreedit {
        text: "かな".into(),
        cursor_begin: 3,
        cursor_end: 6,
    });
    input.apply(&Event::TextInput {
        codepoint: '字' as u32,
    });
    assert_eq!(input.take_text_input(), vec!['字']);

    assert_eq!(
        input.text_preedit(),
        Some(&TextPreedit {
            text: "かな".into(),
            cursor_begin: 3,
            cursor_end: 6,
        })
    );

    input.begin_frame();
    assert_eq!(
        input.text_preedit().map(|preedit| preedit.text.as_str()),
        Some("かな")
    );

    input.apply(&Event::TextPreedit {
        text: String::new(),
        cursor_begin: 0,
        cursor_end: 0,
    });
    assert_eq!(input.text_preedit(), None);

    input.apply(&Event::TextPreedit {
        text: "に".into(),
        cursor_begin: 3,
        cursor_end: 3,
    });
    input.apply(&Event::FocusChanged { focused: false });
    assert_eq!(input.text_preedit(), None);
}

#[test]
fn input_state_exposes_press_and_release_transitions_for_each_frame() {
    use vagus::essential::window::InputState;

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

#[test]
fn input_state_exposes_normalized_mouse_button_transitions() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::Mouse {
        button: MouseButton::Left,
        pressed: true,
    });
    assert!(input.was_mouse_button_pressed(MouseButton::Left));
    assert!(!input.was_mouse_button_released(MouseButton::Left));
    assert!(input.is_mouse_button_down(MouseButton::Left));

    input.apply(&Event::Mouse {
        button: MouseButton::Left,
        pressed: false,
    });
    assert!(input.was_mouse_button_pressed(MouseButton::Left));
    assert!(input.was_mouse_button_released(MouseButton::Left));
    assert!(!input.is_mouse_button_down(MouseButton::Left));

    input.begin_frame();
    assert!(!input.was_mouse_button_pressed(MouseButton::Left));
    assert!(!input.was_mouse_button_released(MouseButton::Left));
}

#[test]
fn typed_mouse_button_transitions_cover_normalized_and_native_codes() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    for button in [
        MouseButton::Left,
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::Back,
        MouseButton::Forward,
        MouseButton::Native(9),
    ] {
        input.apply(&Event::Mouse {
            button,
            pressed: true,
        });
        assert!(
            input.is_mouse_button_down(button),
            "{button:?} should be down"
        );
        assert!(
            input.was_mouse_button_pressed(button),
            "{button:?} should report its press transition"
        );

        input.apply(&Event::Mouse {
            button,
            pressed: false,
        });
        assert!(
            !input.is_mouse_button_down(button),
            "{button:?} should be up"
        );
        assert!(
            input.was_mouse_button_released(button),
            "{button:?} should report its release transition"
        );
        input.begin_frame();
        assert!(!input.was_mouse_button_pressed(button));
        assert!(!input.was_mouse_button_released(button));
    }
}

#[test]
fn input_state_preserves_ordered_ime_deletions_and_commits_per_frame() {
    use vagus::essential::window::InputState;
    use vagus::event::TextInputEvent;

    let mut input = InputState::default();
    input.apply(&Event::TextDeleteSurrounding {
        before_bytes: 3,
        after_bytes: 2,
    });
    input.apply(&Event::TextInput {
        codepoint: '字' as u32,
    });
    input.apply(&Event::TextDeleteSurrounding {
        before_bytes: 0,
        after_bytes: 1,
    });
    input.apply(&Event::TextInput {
        codepoint: '学' as u32,
    });

    assert_eq!(input.take_text_input(), vec!['字', '学']);
    assert_eq!(
        input.take_text_edits(),
        vec![
            TextInputEvent::DeleteSurrounding {
                before_bytes: 3,
                after_bytes: 2,
            },
            TextInputEvent::Commit('字'),
            TextInputEvent::DeleteSurrounding {
                before_bytes: 0,
                after_bytes: 1,
            },
            TextInputEvent::Commit('学'),
        ]
    );

    input.apply(&Event::TextDeleteSurrounding {
        before_bytes: 1,
        after_bytes: 0,
    });
    input.begin_frame();
    assert!(input.take_text_edits().is_empty());
}

#[test]
fn text_input_edits_apply_in_order_without_splitting_utf8() {
    use vagus::event::TextInputEvent;

    let mut text = String::from("a🙂b€c");
    let mut cursor = 5;
    let edits = [
        TextInputEvent::DeleteSurrounding {
            before_bytes: 4,
            after_bytes: 1,
        },
        TextInputEvent::Commit('字'),
        TextInputEvent::DeleteSurrounding {
            before_bytes: 3,
            after_bytes: 3,
        },
        TextInputEvent::Commit('🦀'),
    ];

    for edit in edits {
        edit.apply_to(&mut text, &mut cursor).unwrap();
    }

    assert_eq!(text, "a🦀c");
    assert_eq!(cursor, 5);
}

#[test]
fn text_input_delete_rejects_non_character_boundaries_transactionally() {
    use vagus::event::TextInputEvent;

    let mut text = String::from("a🙂b");
    let mut cursor = 5;
    let error = TextInputEvent::DeleteSurrounding {
        before_bytes: 1,
        after_bytes: 0,
    }
    .apply_to(&mut text, &mut cursor)
    .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(text, "a🙂b");
    assert_eq!(cursor, 5);
}

#[test]
fn input_state_normalizes_pointer_position_by_client_extent() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::PointerMoved { x: 40, y: 25 });
    assert_eq!(
        input.pointer_position_normalized(200, 100),
        Some((0.2, 0.25))
    );

    input.apply(&Event::PointerMoved { x: -10, y: 120 });
    assert_eq!(
        input.pointer_position_normalized(200, 100),
        Some((-0.05, 1.2))
    );
    assert_eq!(input.pointer_position_normalized(0, 100), None);
    assert_eq!(input.pointer_position_normalized(200, 0), None);
}

#[test]
fn input_state_tracks_pointer_enter_and_leave_transitions_per_frame() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::PointerEntered);
    input.apply(&Event::PointerEntered);
    assert!(input.pointer_inside());
    assert!(input.was_pointer_entered());
    assert!(!input.was_pointer_left());

    input.apply(&Event::PointerLeft);
    input.apply(&Event::PointerLeft);
    assert!(!input.pointer_inside());
    assert!(input.was_pointer_entered());
    assert!(input.was_pointer_left());

    input.begin_frame();
    assert!(!input.pointer_inside());
    assert!(!input.was_pointer_entered());
    assert!(!input.was_pointer_left());

    input.apply(&Event::PointerEntered);
    assert!(input.was_pointer_entered());
    assert!(!input.was_pointer_left());
}

#[test]
fn surface_scroll_keeps_native_units_separate_from_wheel_detents() {
    use vagus::essential::window::InputState;

    let mut input = InputState::default();
    input.apply(&Event::SurfaceScroll {
        delta_x: 30.0,
        delta_y: -15.0,
    });

    assert_eq!(input.scroll_delta(), (0.0, 0.0));
    assert_eq!(input.surface_scroll_delta(), (30.0, -15.0));

    input.apply(&Event::Scroll {
        delta_x: 0.5,
        delta_y: -1.0,
    });
    assert_eq!(input.scroll_delta(), (0.5, -1.0));
    assert_eq!(input.surface_scroll_delta(), (30.0, -15.0));

    input.begin_frame();
    assert_eq!(input.scroll_delta(), (0.0, 0.0));
    assert_eq!(input.surface_scroll_delta(), (0.0, 0.0));
}
