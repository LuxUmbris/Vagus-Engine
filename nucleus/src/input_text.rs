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

#[derive(Default)]
pub(crate) struct Utf16InputState {
    pending_high_surrogate: Option<u16>,
}

#[cfg(any(windows, test))]
pub(crate) fn decode_utf16_text(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied())
        .filter_map(Result::ok)
        .collect()
}

#[cfg(any(windows, test))]
pub(crate) fn utf16_offset_to_utf8_byte_offset(units: &[u16], offset: i32) -> Option<i32> {
    let offset = usize::try_from(offset).ok()?;
    if offset > units.len() {
        return None;
    }

    let mut utf16_offset = 0usize;
    let mut utf8_offset = 0usize;
    for decoded in char::decode_utf16(units.iter().copied()) {
        if utf16_offset == offset {
            return i32::try_from(utf8_offset).ok();
        }
        let character = decoded.ok()?;
        let next_utf16_offset = utf16_offset.checked_add(character.len_utf16())?;
        if offset < next_utf16_offset {
            return None;
        }
        utf16_offset = next_utf16_offset;
        utf8_offset = utf8_offset.checked_add(character.len_utf8())?;
    }

    (utf16_offset == offset)
        .then(|| i32::try_from(utf8_offset).ok())
        .flatten()
}

#[cfg(any(windows, test))]
pub(crate) fn decode_utf16_preedit(units: &[u16], cursor_utf16_offset: i32) -> (String, i32, i32) {
    let text = decode_utf16_text(units);
    let cursor = utf16_offset_to_utf8_byte_offset(units, cursor_utf16_offset).unwrap_or(-1);
    (text, cursor, cursor)
}

#[cfg(any(windows, test))]
pub(crate) fn emit_ime_composition_events(
    preedit: Option<(String, i32, i32)>,
    result: Option<&str>,
    mut emit: impl FnMut(crate::Event),
) {
    if let Some(result) = result {
        emit(crate::Event::TextPreedit {
            text: String::new(),
            cursor_begin: -1,
            cursor_end: -1,
        });
        for character in result.chars() {
            emit(crate::Event::TextInput {
                codepoint: character as u32,
            });
        }
    }

    if let Some((text, cursor_begin, cursor_end)) = preedit {
        emit(crate::Event::TextPreedit {
            text,
            cursor_begin,
            cursor_end,
        });
    }
}

impl Utf16InputState {
    pub(crate) fn push(&mut self, unit: u16) -> Option<char> {
        if (0xd800..=0xdbff).contains(&unit) {
            self.pending_high_surrogate = Some(unit);
            return None;
        }

        if (0xdc00..=0xdfff).contains(&unit) {
            return self
                .pending_high_surrogate
                .take()
                .and_then(|high| char::decode_utf16([high, unit]).next().and_then(Result::ok));
        }

        self.pending_high_surrogate = None;
        char::from_u32(unit as u32)
    }

    pub(crate) fn push_codepoint(&mut self, codepoint: u32) -> Option<char> {
        self.reset();
        char::from_u32(codepoint)
    }

    pub(crate) fn reset(&mut self) {
        self.pending_high_surrogate = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_utf16_preedit, decode_utf16_text, emit_ime_composition_events,
        utf16_offset_to_utf8_byte_offset, Utf16InputState,
    };
    use crate::Event;

    #[test]
    fn committed_ime_result_precedes_the_next_preedit() {
        let mut events = Vec::new();
        emit_ime_composition_events(Some(("次".into(), 3, 3)), Some("🙂A"), |event| {
            events.push(event);
        });

        assert_eq!(
            events,
            vec![
                Event::TextPreedit {
                    text: String::new(),
                    cursor_begin: -1,
                    cursor_end: -1,
                },
                Event::TextInput {
                    codepoint: '🙂' as u32,
                },
                Event::TextInput {
                    codepoint: 'A' as u32,
                },
                Event::TextPreedit {
                    text: "次".into(),
                    cursor_begin: 3,
                    cursor_end: 3,
                },
            ]
        );
    }

    #[test]
    fn focus_reset_discards_unmatched_high_surrogate() {
        let mut input = Utf16InputState::default();
        assert_eq!(input.push(0xd83d), None);

        input.reset();

        assert_eq!(input.push(0xde42), None);
        assert_eq!(input.push(b'A' as u16), Some('A'));
    }

    #[test]
    fn decodes_a_valid_surrogate_pair() {
        let mut input = Utf16InputState::default();
        assert_eq!(input.push(0xd83d), None);
        assert_eq!(input.push(0xde42), Some('🙂'));
    }

    #[test]
    fn decodes_direct_unicode_scalars_and_clears_pending_surrogates() {
        let mut input = Utf16InputState::default();
        assert_eq!(input.push(0xd83d), None);
        assert_eq!(input.push_codepoint(0x1f642), Some('🙂'));
        assert_eq!(input.push(0xde42), None);

        assert_eq!(input.push(0xd83d), None);
        assert_eq!(input.push_codepoint(0xd800), None);
        assert_eq!(input.push(0xde42), None);
    }

    #[test]
    fn decodes_complete_utf16_ime_result_strings() {
        assert_eq!(
            decode_utf16_text(&[b'A' as u16, 0x65e5, 0xd83d, 0xde42]),
            "A日🙂"
        );
        assert_eq!(decode_utf16_text(&[0xd800, b'B' as u16, 0xdc00]), "B");
    }

    #[test]
    fn maps_utf16_caret_positions_to_utf8_byte_offsets() {
        let units = [b'A' as u16, 0x65e5, 0xd83d, 0xde42, b'Z' as u16];
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 0), Some(0));
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 1), Some(1));
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 2), Some(4));
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 3), None);
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 4), Some(8));
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 5), Some(9));
        assert_eq!(utf16_offset_to_utf8_byte_offset(&units, 6), None);
        assert_eq!(utf16_offset_to_utf8_byte_offset(&[0xd800], 1), None);
    }

    #[test]
    fn decodes_preedit_text_with_utf8_cursor_offsets() {
        let units = [b'A' as u16, 0x65e5, 0xd83d, 0xde42, b'Z' as u16];
        assert_eq!(decode_utf16_preedit(&units, 4), ("A日🙂Z".into(), 8, 8));
        assert_eq!(decode_utf16_preedit(&units, 3), ("A日🙂Z".into(), -1, -1));
    }
}
