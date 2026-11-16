pub use crate::essential::window::{Event, InputState, Key, MouseButton, RawInput};

pub(super) fn from_native(event: nucleus::Event) -> Event {
    match event {
        nucleus::Event::CloseRequested => Event::CloseRequested,
        nucleus::Event::Resized { width, height } => Event::Resized { width, height },
        nucleus::Event::Moved { x, y } => Event::WindowMoved { x, y },
        nucleus::Event::Focused { focused } => Event::FocusChanged { focused },
        nucleus::Event::Key { code, pressed } => Event::Key(Key { code, pressed }),
        nucleus::Event::PointerMoved { x, y } => Event::PointerMoved { x, y },
        nucleus::Event::PointerButton { button, pressed } => Event::Mouse {
            button: MouseButton::Native(button),
            pressed,
        },
        nucleus::Event::PointerEntered => Event::PointerEntered,
        nucleus::Event::PointerLeft => Event::PointerLeft,
        nucleus::Event::Scroll { delta_x, delta_y } => Event::Scroll { delta_x, delta_y },
        nucleus::Event::TextInput { codepoint } => Event::TextInput { codepoint },
        nucleus::Event::RawInput { kind, code } => Event::Raw(RawInput { kind, code }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Key, MouseButton, RawInput, from_native};
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
    }
}
