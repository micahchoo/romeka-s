//! IPTC IIM datasets (record 2), as photo desks and Photoshop wrote them,
//! and the Photoshop resource block that carries them in a JPEG.

use super::{Metadata, decode_text};

/// The datasets read: the IIM number, the key, and whether it repeats.
const DATASETS: [(u8, &str, bool); 7] = [
    (5, "IPTC Object Name", false),
    (25, "IPTC Keywords", true),
    (55, "IPTC Date Created", false),
    (80, "IPTC By-line", true),
    (90, "IPTC City", false),
    (101, "IPTC Country", false),
    (120, "IPTC Caption-Abstract", false),
];

/// The Photoshop resource that holds IPTC-NAA data.
const IPTC_RESOURCE: u16 = 0x0404;

/// The IIM block inside Photoshop resources (`8BIM` records).
pub(super) fn from_resources(mut block: &[u8]) -> Option<&[u8]> {
    while block.len() >= 12 && block.starts_with(b"8BIM") {
        let id = u16::from_be_bytes([block[4], block[5]]);
        let name = usize::from(block[6]) + 1;
        let name = name + name % 2;
        let rest = block.get(6 + name..)?;
        let size = usize::try_from(u32::from_be_bytes(rest.get(..4)?.try_into().ok()?)).ok()?;
        let data = rest.get(4..4 + size)?;
        if id == IPTC_RESOURCE {
            return Some(data);
        }
        block = rest.get(4 + size + size % 2..).unwrap_or_default();
    }
    None
}

/// The datasets of an IIM block into `tags`. Text that is not UTF-8 is
/// read as Windows-1252, whatever record 1 declares, and marked so.
pub(super) fn read(mut block: &[u8], tags: &mut Metadata) {
    while block.len() >= 5 && block[0] == 0x1C {
        let (record, dataset) = (block[1], block[2]);
        let mut length = usize::from(u16::from_be_bytes([block[3], block[4]]));
        let mut at = 5;
        if length & 0x8000 != 0 {
            let width = length & 0x7FFF;
            let Some(bytes) = block.get(5..5 + width).filter(|_| width <= 4) else {
                return;
            };
            length = bytes
                .iter()
                .fold(0, |length, byte| (length << 8) | usize::from(*byte));
            at += width;
        }
        let Some(value) = block.get(at..at + length) else {
            return;
        };
        block = &block[at + length..];
        if record != 2 {
            continue;
        }
        let Some((_, key, repeats)) = DATASETS.iter().find(|(number, ..)| *number == dataset)
        else {
            continue;
        };
        let Some((mut text, windows_1252)) = decode_text(value) else {
            continue;
        };
        if dataset == 55 {
            text = iso_date(&text).unwrap_or(text);
        }
        let entry = tags.entry(key).or_default();
        if *repeats || entry.values.is_empty() {
            entry.values.push(text);
            entry.windows_1252 |= windows_1252;
        }
    }
}

/// `CCYYMMDD` as ISO, a `00` month or day left off (IIM's "unknown").
fn iso_date(text: &str) -> Option<String> {
    if text.len() != 8 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (year, month, day) = (&text[..4], &text[4..6], &text[6..]);
    Some(match (month, day) {
        ("00", _) => year.to_string(),
        (_, "00") => format!("{year}-{month}"),
        _ => format!("{year}-{month}-{day}"),
    })
}
