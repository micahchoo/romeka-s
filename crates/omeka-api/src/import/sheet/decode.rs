//! Bytes to text: the encoding the plan names, or a guess with a confidence.
//!
//! The guess goes, in order: a byte order mark; the NUL pattern of UTF-16;
//! UTF-8, also when a few bytes are not (a 1252 byte in a UTF-8 file must
//! not spoil every Devanagari cell); then `chardetng`. A guess the reader
//! has no decoder for is refused by its name (`encoding-unsupported`).

use encoding_rs::{UTF_16BE, UTF_16LE, WINDOWS_1252};
use serde::Serialize;

use crate::import::plan::{Encoding, EncodingChoice};

/// The encoding a sheet was read in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodingRead {
    pub encoding: Encoding,
    /// For `auto`, how sure the guess is, 0 to 1. `None` when the plan
    /// named the encoding.
    pub confidence: Option<f32>,
}

/// The text without a byte order mark, and the encoding it was read in.
///
/// # Errors
/// For `auto`, the name of a detected encoding the reader cannot decode.
pub(super) fn decode(
    bytes: &[u8],
    choice: EncodingChoice,
) -> Result<(String, EncodingRead), &'static str> {
    let (encoding, confidence) = match choice {
        EncodingChoice::Is(encoding) => (encoding, None),
        EncodingChoice::Auto(_) => {
            let (encoding, confidence) = guess(bytes)?;
            (encoding, Some(confidence))
        }
    };
    let text = match encoding {
        Encoding::Utf8 => {
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            String::from_utf8_lossy(bytes).into_owned()
        }
        Encoding::Utf16Le => {
            let bytes = bytes.strip_prefix(b"\xFF\xFE").unwrap_or(bytes);
            UTF_16LE.decode_without_bom_handling(bytes).0.into_owned()
        }
        Encoding::Utf16Be => {
            let bytes = bytes.strip_prefix(b"\xFE\xFF").unwrap_or(bytes);
            UTF_16BE.decode_without_bom_handling(bytes).0.into_owned()
        }
        Encoding::Windows1252 => WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
        Encoding::Iso88591 => bytes.iter().copied().map(char::from).collect(),
    };
    Ok((
        text,
        EncodingRead {
            encoding,
            confidence,
        },
    ))
}

/// How much of a file the UTF-16 test looks at.
const SAMPLE: usize = 4096;

fn guess(bytes: &[u8]) -> Result<(Encoding, f32), &'static str> {
    if bytes.starts_with(b"\xFF\xFE\0\0") {
        return Err("UTF-32LE");
    }
    if bytes.starts_with(b"\0\0\xFE\xFF") {
        return Err("UTF-32BE");
    }
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return Ok((Encoding::Utf8, 1.0));
    }
    if bytes.starts_with(b"\xFF\xFE") {
        return Ok((Encoding::Utf16Le, 1.0));
    }
    if bytes.starts_with(b"\xFE\xFF") {
        return Ok((Encoding::Utf16Be, 1.0));
    }
    if let Some(found) = utf16_by_nuls(&bytes[..bytes.len().min(SAMPLE)]) {
        return Ok(found);
    }
    if let Some(confidence) = mostly_utf8(bytes) {
        return Ok((Encoding::Utf8, confidence));
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    let detected = detector.guess(None, chardetng::Utf8Detection::Deny);
    if detected == WINDOWS_1252 {
        let text = WINDOWS_1252.decode_without_bom_handling(bytes).0;
        return Ok((Encoding::Windows1252, letter_share(&text) * 0.9));
    }
    Err(detected.name())
}

/// UTF-16 without a mark: text in Latin script, digits or delimiters puts a
/// NUL in every other byte, and text in UTF-8 or a code page has none.
fn utf16_by_nuls(sample: &[u8]) -> Option<(Encoding, f32)> {
    let pairs = sample.len() / 2;
    let even = sample.iter().step_by(2).filter(|b| **b == 0).count();
    let odd = sample
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|b| **b == 0)
        .count();
    let nuls = even + odd;
    if pairs == 0 || nuls * 20 < pairs {
        return None;
    }
    let (encoding, side) = if odd > even {
        (Encoding::Utf16Le, odd)
    } else {
        (Encoding::Utf16Be, even)
    };
    let share = ratio(side, nuls);
    (share >= 0.9).then_some((encoding, share))
}

/// `Some(confidence)` when the bytes are UTF-8, or when the characters that
/// are valid multi-byte UTF-8 outnumber the bytes that are not.
fn mostly_utf8(bytes: &[u8]) -> Option<f32> {
    let mut valid = 0;
    let mut invalid = 0;
    for chunk in bytes.utf8_chunks() {
        valid += chunk.valid().chars().filter(|c| !c.is_ascii()).count();
        invalid += chunk.invalid().len();
    }
    if invalid == 0 {
        return Some(1.0);
    }
    (valid >= invalid).then(|| ratio(valid, valid + invalid))
}

/// The share of non-ASCII characters that are letters: high for a real
/// Western text, low for bytes of another code page read as 1252.
fn letter_share(text: &str) -> f32 {
    let high = text.chars().filter(|c| !c.is_ascii());
    let (letters, all) = high.fold((0, 0), |(letters, all), c| {
        (letters + usize::from(c.is_alphabetic()), all + 1)
    });
    if all == 0 { 1.0 } else { ratio(letters, all) }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(part: usize, whole: usize) -> f32 {
    part as f32 / whole as f32
}
