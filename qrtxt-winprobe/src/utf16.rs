//! Incremental UTF-16 decoding for console key characters.

/// Decodes the console's UTF-16 code units into UTF-8 bytes.
///
/// Console key records carry one UTF-16 code unit at a time, so a character
/// outside the BMP arrives as a high surrogate followed by a low surrogate.
#[derive(Debug, Default)]
pub(crate) struct Utf16Decoder {
    pending_high: Option<u16>,
}

impl Utf16Decoder {
    /// Push `unit` `repeat` times, appending its UTF-8 encoding to `out`.
    pub(crate) fn push(&mut self, unit: u16, repeat: u16, out: &mut Vec<u8>) {
        for _ in 0..repeat.max(1) {
            self.push_one(unit, out);
        }
    }

    /// Flush a dangling high surrogate as a replacement character.
    pub(crate) fn finish(&mut self, out: &mut Vec<u8>) {
        if self.pending_high.take().is_some() {
            self.encode(char::REPLACEMENT_CHARACTER, out);
        }
    }

    fn push_one(&mut self, unit: u16, out: &mut Vec<u8>) {
        if let Some(high) = self.pending_high.take() {
            if (0xDC00..=0xDFFF).contains(&unit) {
                let code =
                    0x1_0000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(unit) - 0xDC00);
                self.encode(
                    char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER),
                    out,
                );
                return;
            }
            self.encode(char::REPLACEMENT_CHARACTER, out);
        }
        if (0xD800..=0xDBFF).contains(&unit) {
            self.pending_high = Some(unit);
        } else if (0xDC00..=0xDFFF).contains(&unit) {
            self.encode(char::REPLACEMENT_CHARACTER, out);
        } else {
            self.encode(
                char::from_u32(u32::from(unit)).unwrap_or(char::REPLACEMENT_CHARACTER),
                out,
            );
        }
    }

    fn encode(&self, character: char, out: &mut Vec<u8>) {
        let mut bytes = [0u8; 4];
        out.extend_from_slice(character.encode_utf8(&mut bytes).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(units: &[(u16, u16)]) -> Vec<u8> {
        let mut decoder = Utf16Decoder::default();
        let mut out = Vec::new();
        for &(unit, repeat) in units {
            decoder.push(unit, repeat, &mut out);
        }
        decoder.finish(&mut out);
        out
    }

    #[test]
    fn ascii_units_decode() {
        assert_eq!(decode(&[(u16::from(b'a'), 1), (u16::from(b'b'), 1)]), b"ab");
    }

    #[test]
    fn surrogate_pairs_decode_to_one_character() {
        assert_eq!(decode(&[(0xD83D, 1), (0xDE00, 1)]), "😀".as_bytes());
    }

    #[test]
    fn a_lone_high_surrogate_becomes_a_replacement_character() {
        assert_eq!(decode(&[(0xD83D, 1)]), "\u{FFFD}".as_bytes());
    }

    #[test]
    fn a_lone_low_surrogate_becomes_a_replacement_character() {
        assert_eq!(decode(&[(0xDE00, 1)]), "\u{FFFD}".as_bytes());
    }

    #[test]
    fn a_high_surrogate_followed_by_ascii_is_replaced() {
        assert_eq!(
            decode(&[(0xD83D, 1), (u16::from(b'a'), 1)]),
            "\u{FFFD}a".as_bytes()
        );
    }

    #[test]
    fn repeat_count_repeats_the_unit() {
        assert_eq!(decode(&[(u16::from(b'x'), 3)]), b"xxx");
    }

    #[test]
    fn a_zero_repeat_count_still_emits_once() {
        assert_eq!(decode(&[(u16::from(b'x'), 0)]), b"x");
    }

    #[test]
    fn multi_byte_units_encode_as_utf8() {
        assert_eq!(decode(&[(0x4E2D, 1)]), "中".as_bytes());
    }
}
