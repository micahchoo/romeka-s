//! Windows-1252, by hand: the code page old spreadsheets are saved in.
//!
//! Bytes 0x00..=0x7F and 0xA0..=0xFF are the same code points in Latin-1;
//! only 0x80..=0x9F differ, and [`HIGH`] lists them. The five bytes the code
//! page leaves undefined map to the C1 control of the same value, as the WHATWG
//! encoding standard does, so decoding never fails and encoding is its inverse.
//!
//! Text goes wrong in two ways. A file saved in Windows-1252 is not UTF-8,
//! so [`decode`] reads it. A UTF-8 file that passed once through a
//! Windows-1252 reader holds "CafÃ©" for "Café", and [`repair`] undoes that.

/// The code points of bytes 0x80..=0x9F.
const HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

fn decode_byte(byte: u8) -> char {
    match byte {
        0x80..=0x9F => HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

fn encode_char(character: char) -> Option<u8> {
    let code = u32::from(character);
    if code < 0x80 || (0xA0..=0xFF).contains(&code) {
        return u8::try_from(code).ok();
    }
    HIGH.iter()
        .position(|&c| c == character)
        .and_then(|index| u8::try_from(0x80 + index).ok())
}

/// Read bytes written in Windows-1252.
#[must_use]
pub fn decode(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| decode_byte(byte)).collect()
}

/// Undo one pass of UTF-8 read as Windows-1252: "CafÃ©" becomes "Café".
///
/// `None` when the text is not such mojibake: plain ASCII, a character the
/// code page has no byte for, or bytes that are not UTF-8 ("Café" itself).
#[must_use]
pub fn repair(text: &str) -> Option<String> {
    if text.is_ascii() {
        return None;
    }
    let bytes = text.chars().map(encode_char).collect::<Option<Vec<u8>>>()?;
    let repaired = String::from_utf8(bytes).ok()?;
    (repaired != text).then_some(repaired)
}

/// Whether a cell shows the marks of a wrong text encoding: bytes that were
/// not UTF-8 (U+FFFD), or mojibake [`repair`] can undo.
#[must_use]
pub fn looks_misdecoded(text: &str) -> bool {
    text.contains('\u{FFFD}') || repair(text).is_some()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn decodes_the_bytes_latin1_and_the_code_page_disagree_on() {
        assert_eq!(decode(b"Caf\xE9 \x80 \x93quoted\x94"), "Café € “quoted”");
    }

    #[test]
    fn every_byte_round_trips() {
        for byte in 0..=u8::MAX {
            assert_eq!(encode_char(decode_byte(byte)), Some(byte), "byte {byte:#x}");
        }
    }

    #[test]
    fn repairs_mojibake_and_leaves_good_text_alone() {
        assert_eq!(repair("CafÃ© sign").as_deref(), Some("Café sign"));
        assert_eq!(repair("â€œHarbourâ€\u{9d}").as_deref(), Some("“Harbour”"));
        assert_eq!(repair("Café sign"), None);
        assert_eq!(repair("Brass lamp"), None);
        assert_eq!(repair("मुंबई"), None);
    }

    #[test]
    fn a_replacement_character_or_mojibake_looks_misdecoded() {
        assert!(looks_misdecoded("Caf\u{FFFD} sign"));
        assert!(looks_misdecoded("CafÃ© sign"));
        assert!(!looks_misdecoded("Café sign"));
    }
}
