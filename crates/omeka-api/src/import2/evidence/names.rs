//! File names that are not UTF-8: decoded, never dropped.
//!
//! Such a name comes from a ZIP made on old Windows or DOS (cp437) or from a
//! Western code page (Latin-1). The bytes alone cannot say which, so
//! [`decode`] scores both readings by how many of the high bytes become
//! Latin letters, and Latin-1 wins a tie. `Año` in cp437 is `A\xA4o`: cp437
//! reads `ñ`, Latin-1 reads `¤`, so cp437 wins. `café` in Latin-1 is
//! `caf\xE9`: Latin-1 reads `é`, cp437 reads the Greek `Θ`, so Latin-1 wins.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NameEncoding {
    Cp437,
    Latin1,
}

/// cp437 bytes 0x80..=0xFF.
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{A0}',
];

fn cp437(byte: u8) -> char {
    if byte < 0x80 {
        char::from(byte)
    } else {
        CP437_HIGH[usize::from(byte - 0x80)]
    }
}

fn is_latin_letter(character: char) -> bool {
    matches!(character, 'À'..='ÿ' | 'Ā'..='ɏ') && character != '×' && character != '÷'
}

/// The name read in the encoding that makes more of its high bytes Latin
/// letters.
#[must_use]
pub fn decode(bytes: &[u8]) -> (String, NameEncoding) {
    let latin1: String = bytes.iter().copied().map(char::from).collect();
    let dos: String = bytes.iter().copied().map(cp437).collect();
    let score = |text: &str| text.chars().filter(|c| is_latin_letter(*c)).count();
    if score(&dos) > score(&latin1) {
        (dos, NameEncoding::Cp437)
    } else {
        (latin1, NameEncoding::Latin1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_read_in_the_encoding_that_gives_it_letters() {
        assert_eq!(
            decode(b"A\xA4o 1950"),
            ("Año 1950".to_string(), NameEncoding::Cp437)
        );
        assert_eq!(
            decode(b"caf\xE9.jpg"),
            ("café.jpg".to_string(), NameEncoding::Latin1)
        );
        assert_eq!(
            decode(b"\x8Avora.tif"),
            ("èvora.tif".to_string(), NameEncoding::Cp437)
        );
        assert_eq!(decode(b"\xFF"), ("ÿ".to_string(), NameEncoding::Latin1));
    }
}
