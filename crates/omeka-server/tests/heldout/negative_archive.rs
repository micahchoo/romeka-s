//! Frozen held-out shape, written by the keeper before the proposer was built: never edit it or `expected()`.
//!
//! The Ashby Courier's negatives: a weekly paper's photographic jobs of 1974
//! and 1975, scanned by volunteers. Each job's envelope is one folder, named
//! by its job number and the assignment, under the year. A job shot on
//! several rolls has its frames numbered from 1 again on each roll. The
//! photographers' shot log, a CSV saved from an old office PC in Windows
//! code page 1252, has a row per job. It has rows for jobs whose envelopes
//! were empty, a transposed job number, and no row for one job whose log
//! page is lost. One volunteer typed an underscore in a folder name, another
//! did not pad the frame numbers. Loose negatives with no envelope wait in
//! `_unsorted`.

use super::{Item, Outcome, Shape, kind};

/// The keeper's v3 plan of this archive: proof that the outcome is reachable.
pub const PLAN: &str = include_str!("fixtures/negative_archive.json");

/// One job envelope on disk.
struct Job {
    year: &'static str,
    /// The folder's name, as the volunteer typed it.
    folder: &'static str,
    /// The job number as the paper wrote it.
    number: &'static str,
    /// Frames per roll.
    rolls: &'static [usize],
    /// Whether frame numbers are padded to two places.
    padded: bool,
    /// The contact sheet's name, if it was scanned.
    contact: Option<&'static str>,
}

const fn job(
    year: &'static str,
    folder: &'static str,
    number: &'static str,
    rolls: &'static [usize],
    padded: bool,
    contact: Option<&'static str>,
) -> Job {
    Job {
        year,
        folder,
        number,
        rolls,
        padded,
        contact,
    }
}

/// The envelopes, in job order.
const JOBS: [Job; 9] = [
    job(
        "1974",
        "74-112 Mill fire",
        "74-112",
        &[8, 5],
        true,
        Some("contact sheet.jpg"),
    ),
    job("1974", "74-113 Council meeting", "74-113", &[6], true, None),
    job(
        "1974",
        "74-118 Flood at Ferry Lane",
        "74-118",
        &[12, 4],
        true,
        Some("contact sheet.jpg"),
    ),
    job("1974", "74_131 Spring show", "74-131", &[7], true, None),
    job("1974", "74-140 Café opening", "74-140", &[5], true, None),
    job(
        "1975",
        "75-003 Snow on the moor",
        "75-003",
        &[12],
        false,
        None,
    ),
    job(
        "1975",
        "75-004 Ashby United v Thornley",
        "75-004",
        &[10, 10, 3],
        true,
        None,
    ),
    job(
        "1975",
        "75-009 Dr Okafor's retirement",
        "75-009",
        &[4],
        true,
        Some("contact sheet.JPG"),
    ),
    job("1975", "75-021 Royal visit", "75-021", &[9], true, None),
];

/// The shot log's rows, in its order: job, date, assignment, photographer,
/// rolls, page published on. Text is as the log holds it, decoded.
const LOG: [[&str; 6]; 11] = [
    [
        "74-110",
        "25/02/1974",
        "Ashby Rotary dinner",
        "P. Marsh",
        "1",
        "p.5",
    ],
    [
        "74-112",
        "02/03/1974",
        "Mill fire, Station Road",
        "P. Marsh",
        "2",
        "p.1",
    ],
    [
        "74-113",
        "04/03/1974",
        "Council meeting",
        "J. Kowalski",
        "1",
        "",
    ],
    [
        "74-115",
        "08/03/1974",
        "Lambing at Hill Farm",
        "P. Marsh",
        "1",
        "p.9",
    ],
    [
        "74-118",
        "16/03/1974",
        "Flood at Ferry Lane",
        "J. Kowalski",
        "2",
        "p.1",
    ],
    [
        "74-131",
        "20/04/1974",
        "Spring show",
        "P. Marsh",
        "1",
        "p.7",
    ],
    // The job number is transposed: the envelope is 74-140.
    [
        "47-140",
        "11/05/1974",
        "Café opening, Market Street",
        "J. Kowalski",
        "1",
        "p.4",
    ],
    [
        "75-003",
        "10/01/1975",
        "Snow on the moor",
        "P. Marsh",
        "1",
        "p.1",
    ],
    [
        "75-004",
        "11/01/1975",
        "Ashby United v Thornley",
        "J. Kowalski",
        "3",
        "p.16",
    ],
    [
        "75-009",
        "24/01/1975",
        "Dr Okafor’s retirement",
        "P. Marsh",
        "1",
        "p.3",
    ],
    [
        "75-012",
        "01/02/1975",
        "Ashby Players, \"The Rivals\"",
        "J. Kowalski",
        "1",
        "",
    ],
];

const TRANSPOSED: (&str, &str) = ("47-140", "74-140");
const LOG_FILE: &str = "Shot log 1974-75.csv";
const NOTES: &str = "Scanning notes.txt";
const UNSORTED: [&str; 4] = [
    "Negatives/_unsorted/scan0001.tif",
    "Negatives/_unsorted/scan0002.tif",
    "Negatives/_unsorted/scan0003.tif",
    "Negatives/_unsorted/scan0004.tif",
];
const SYSTEM: [(&str, &[u8]); 2] = [
    ("Negatives/.DS_Store", b"\x00\x00\x00\x01Bud1"),
    (
        "Negatives/1975/75-004 Ashby United v Thornley/Thumbs.db",
        b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00",
    ),
];

impl Job {
    fn path(&self) -> String {
        format!("Negatives/{}/{}", self.year, self.folder)
    }

    /// The scanned frames, roll by roll, then the contact sheet.
    fn files(&self) -> Vec<String> {
        let mut files = Vec::new();
        for (roll, frames) in (1..).zip(self.rolls) {
            for frame in 1..=*frames {
                let name = if self.padded {
                    format!("R{roll}-{frame:02}.tif")
                } else {
                    format!("R{roll}-{frame}.tif")
                };
                files.push(format!("{}/{name}", self.path()));
            }
        }
        files.extend(self.contact.map(|name| format!("{}/{name}", self.path())));
        files
    }

    fn row(&self) -> Option<&'static [&'static str; 6]> {
        let logged = if self.number == TRANSPOSED.1 {
            TRANSPOSED.0
        } else {
            self.number
        };
        LOG.iter().find(|row| row[0] == logged)
    }
}

/// A CSV cell, quoted when it must be.
fn quoted(text: &str) -> String {
    if text.contains([',', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// The log in code page 1252, with CRLF line ends.
fn shot_log() -> Vec<u8> {
    let mut text = String::from("Job,Date,Assignment,Photographer,Rolls,Published\r\n");
    for row in LOG {
        let cells: Vec<String> = row.iter().map(|cell| quoted(cell)).collect();
        text.push_str(&cells.join(","));
        text.push_str("\r\n");
    }
    text.chars()
        .map(|c| match c {
            'é' => 0xE9,
            '’' => 0x92,
            c if c.is_ascii() => c as u8,
            c => panic!("{c} is not in the log's code page here"),
        })
        .collect()
}

pub fn shape() -> Shape {
    let mut shape = Shape::default();
    for job in &JOBS {
        for path in job.files() {
            let header = if path.ends_with(".tif") {
                kind::TIFF
            } else {
                kind::JPEG
            };
            shape.object(&path, header);
        }
    }
    for path in UNSORTED {
        shape.object(path, kind::TIFF);
    }
    for (path, bytes) in SYSTEM {
        shape.raw(path, bytes);
    }
    shape.raw(
        NOTES,
        b"Scanned at 2400 dpi on the Epson, 2022-23.\nEnvelopes 74-110, 74-115 and 75-012 were empty.\n",
    );
    shape.raw(LOG_FILE, &shot_log());
    shape
}

/// The slots `expected()` names values for.
pub const SLOTS: [&str; 2] = ["dcterms:date", "dcterms:creator"];

/// One item per envelope, identified by its job number as the paper wrote
/// it (the envelope's number where the log transposed it), its frames in
/// roll and frame order, then its contact sheet. Its log row gives its
/// title, its date and its photographer. A job with no log row is titled by
/// the assignment in its folder name. Each item is in the set of its year.
/// The log's rows for empty envelopes describe nothing. Left out: the loose
/// negatives, the system files, the scanning notes and the log itself.
pub fn expected() -> Outcome {
    let mut outcome = Outcome::default();
    for job in &JOBS {
        let item = match job.row() {
            Some(row) => {
                let mut item = Item::new(row[2], &job.files(), &[job.year]);
                item.value("dcterms:date", row[1])
                    .value("dcterms:creator", row[3]);
                item
            }
            None => {
                let (_, assignment) = job.folder.split_once(' ').unwrap();
                Item::new(assignment, &job.files(), &[job.year])
            }
        };
        outcome.items.insert(job.number.to_string(), item);
    }
    outcome
        .left_out
        .extend(UNSORTED.iter().map(|path| path.to_string()));
    outcome
        .left_out
        .extend(SYSTEM.iter().map(|(path, _)| path.to_string()));
    outcome
        .left_out
        .extend([NOTES.to_string(), LOG_FILE.to_string()]);
    outcome
}
