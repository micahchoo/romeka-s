//! Frozen held-out shape, written by the keeper before the proposer was built: never edit it or `expected()`.
//!
//! The Patil Family Papers: a family's letters, legal papers and photographs,
//! boxed and foldered by a volunteer, then scanned page by page. A container
//! list describes each folder by its reference, which the folder names pad
//! with zeros and the list does not. Box 01 has no folder 05: it was merged
//! into folder 04 before the list was typed. Box 02's folder 11 is listed and
//! was never scanned. A map too large for a box is in `Oversize/`. The
//! scanning project kept its own log and checksums beside the boxes.

use super::{Item, Outcome, Shape, kind};

/// The keeper's v3 plan of this archive: proof that the outcome is reachable.
pub const PLAN: &str = include_str!("fixtures/paper_archive.json");

/// One folder of the container list.
struct Folder {
    boxed: u8,
    folder: u8,
    /// The volunteer's few words on the folder's label.
    short: &'static str,
    title: &'static str,
    dates: &'static str,
    series: &'static str,
}

const fn folder(
    boxed: u8,
    folder: u8,
    short: &'static str,
    title: &'static str,
    dates: &'static str,
    series: &'static str,
) -> Folder {
    Folder {
        boxed,
        folder,
        short,
        title,
        dates,
        series,
    }
}

const CORRESPONDENCE: &str = "Correspondence";
const LEGAL: &str = "Legal";
const PHOTOGRAPHS: &str = "Photographs";

/// The 23 folders on disk, in box order, then folder order.
const SCANNED: [Folder; 23] = [
    folder(
        1,
        1,
        "Letters from Anand",
        "Letters from Anand Patil to his parents, Pune",
        "1947-1952",
        CORRESPONDENCE,
    ),
    folder(
        1,
        2,
        "Letters to Sushila",
        "Letters to Sushila Patil from her sisters",
        "1950-1958",
        CORRESPONDENCE,
    ),
    folder(
        1,
        3,
        "Wedding cards",
        "Wedding invitations and greeting cards",
        "c. 1955",
        CORRESPONDENCE,
    ),
    folder(
        1,
        4,
        "Bombay letters",
        "Letters from relatives in Bombay, with two telegrams",
        "1953-1961",
        CORRESPONDENCE,
    ),
    folder(
        1,
        6,
        "Letters, Nairobi",
        "Letters from Vinod Patil in Nairobi",
        "1962-1970",
        CORRESPONDENCE,
    ),
    folder(
        1,
        7,
        "Postcards",
        "Picture postcards sent to the family",
        "n.d.",
        CORRESPONDENCE,
    ),
    folder(
        1,
        8,
        "School letters",
        "Letters about the children's schooling",
        "1964-1969",
        CORRESPONDENCE,
    ),
    folder(
        1,
        9,
        "Condolences 1971",
        "Letters of condolence on the death of Anand Patil",
        "1971",
        CORRESPONDENCE,
    ),
    folder(
        2,
        1,
        "Misc letters",
        "Family letters, writers not identified",
        "n.d.",
        CORRESPONDENCE,
    ),
    folder(
        2,
        2,
        "Aerogrammes",
        "Aerogrammes from England",
        "1972-1975",
        CORRESPONDENCE,
    ),
    folder(
        2,
        3,
        "Letters from Meera",
        "Letters from Meera Patil, Leicester",
        "1976-1984",
        CORRESPONDENCE,
    ),
    folder(
        2,
        4,
        "Land deed",
        "Deed of sale for the Satara land",
        "1949",
        LEGAL,
    ),
    folder(
        2,
        5,
        "Mutation records",
        "Village land mutation records",
        "c. 1950",
        LEGAL,
    ),
    folder(
        2,
        6,
        "Will of Anand",
        "Will of Anand Patil and probate papers",
        "1968-1972",
        LEGAL,
    ),
    folder(
        2,
        7,
        "Tax receipts",
        "Land revenue receipts",
        "1955-1965",
        LEGAL,
    ),
    folder(
        2,
        8,
        "Court case",
        "Papers of the boundary case at the Satara court",
        "1958-1963",
        LEGAL,
    ),
    folder(
        2,
        9,
        "Ration cards",
        "Ration cards and household papers",
        "c. 1960",
        LEGAL,
    ),
    folder(
        2,
        10,
        "Insurance",
        "Life insurance policies",
        "1960-1975",
        LEGAL,
    ),
    folder(
        3,
        1,
        "Studio portraits",
        "Studio portraits of the Patil family",
        "c. 1950",
        PHOTOGRAPHS,
    ),
    folder(
        3,
        2,
        "Wedding 1955",
        "Photographs of the wedding of Anand and Sushila",
        "1955",
        PHOTOGRAPHS,
    ),
    folder(
        3,
        3,
        "Satara farm",
        "Photographs of the farm at Satara",
        "1960-1965",
        PHOTOGRAPHS,
    ),
    folder(
        3,
        4,
        "Nairobi visit",
        "Photographs of the visit to Nairobi",
        "1966",
        PHOTOGRAPHS,
    ),
    folder(
        3,
        5,
        "Snapshots",
        "Loose snapshots, people not identified",
        "n.d.",
        PHOTOGRAPHS,
    ),
];

/// Listed, never scanned: no folder on disk.
const UNSCANNED: Folder = folder(
    2,
    11,
    "Loan papers",
    "Papers of the cooperative bank loan",
    "1966",
    LEGAL,
);

/// The oversize map: two sheets in `Oversize/`, listed as Box 3, Folder 6.
const MAP: Folder = folder(
    3,
    6,
    "survey map",
    "Survey map of the Satara land",
    "1948",
    PHOTOGRAPHS,
);
const MAP_SHEETS: [&str; 2] = ["survey map sheet 1.tif", "survey map sheet 2.tif"];

/// Pages the scanner operator named by hand, wrongly: (folder ref, page, name).
const MISNAMED: [(&str, usize, &str); 3] = [
    ("PFP-01-02", 5, "0005.TIF"),
    ("PFP-01-07", 3, "0003.tiff"),
    ("PFP-02-08", 3, "003.tif"),
];

/// A page found after the folder was scanned, put after page 4. The list's
/// extent was counted before, so it does not count this page.
const FOUND_LATER: (&str, &str) = ("PFP-02-04", "0004a.tif");

const THUMBS: (&str, &[u8]) = ("PFP-01-06", b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00");

const PROJECT: &str = "Scanning project";
const LIST: &str = "container-list.xlsx";

impl Folder {
    /// The reference as the folder names spell it.
    fn reference(&self) -> String {
        format!("PFP-{:02}-{:02}", self.boxed, self.folder)
    }

    /// The reference as the volunteer typed it in the list.
    fn listed(&self) -> String {
        format!("PFP-{}-{}", self.boxed, self.folder)
    }

    fn path(&self) -> String {
        format!("Box {:02}/{} {}", self.boxed, self.reference(), self.short)
    }
}

/// The pages of the k-th folder on disk (from 1): N = 2 + (7k mod 19).
fn leaves(k: usize) -> usize {
    2 + (7 * k) % 19
}

/// The files of a scanned folder, in page order, by name.
fn pages(k: usize, folder: &Folder) -> Vec<String> {
    let reference = folder.reference();
    let mut names = Vec::new();
    for page in 1..=leaves(k) {
        let misnamed = MISNAMED
            .iter()
            .find(|(at, number, _)| *at == reference && *number == page);
        names.push(
            misnamed.map_or_else(|| format!("{page:04}.tif"), |(_, _, name)| name.to_string()),
        );
        if FOUND_LATER.0 == reference && page == 4 {
            names.push(FOUND_LATER.1.to_string());
        }
    }
    names
}

fn scanned() -> impl Iterator<Item = (usize, &'static Folder)> {
    (1..).zip(SCANNED.iter())
}

fn map_files() -> Vec<String> {
    MAP_SHEETS
        .iter()
        .map(|sheet| format!("Oversize/{} {sheet}", MAP.reference()))
        .collect()
}

/// The container list, one worksheet named `Container list`.
fn container_list() -> Vec<u8> {
    let mut rows: Vec<(&Folder, String)> = scanned()
        .map(|(k, folder)| (folder, format!("{} leaves", leaves(k))))
        .collect();
    rows.push((&UNSCANNED, "1 folder".to_string()));
    rows.push((&MAP, "2 sheets".to_string()));
    rows.sort_by_key(|(folder, _)| (folder.boxed, folder.folder));
    let mut book = rust_xlsxwriter::Workbook::new();
    let page = book.add_worksheet();
    page.set_name("Container list").unwrap();
    let header = ["Ref", "Box", "Folder", "Title", "Dates", "Series", "Extent"];
    for (col, text) in (0_u16..).zip(header) {
        page.write(0, col, text).unwrap();
    }
    for (line, (folder, extent)) in (1_u32..).zip(&rows) {
        page.write(line, 0, folder.listed()).unwrap();
        page.write(line, 1, f64::from(folder.boxed)).unwrap();
        page.write(line, 2, f64::from(folder.folder)).unwrap();
        page.write(line, 3, folder.title).unwrap();
        page.write(line, 4, folder.dates).unwrap();
        page.write(line, 5, folder.series).unwrap();
        page.write(line, 6, extent.as_str()).unwrap();
    }
    book.save_to_buffer().unwrap()
}

fn project_files(shape: &mut Shape) {
    shape.raw(
        &format!("{PROJECT}/scan-log.txt"),
        b"2019-03-02 Box 01 scanned, 300 dpi\n2019-03-09 Box 02 scanned, 300 dpi\n\
          2019-03-16 Box 03 scanned, 600 dpi\n2019-03-23 Oversize scanned in two sheets\n",
    );
    let mut sums = String::new();
    for (k, folder) in scanned().filter(|(_, folder)| folder.boxed == 1) {
        for name in pages(k, folder) {
            let path = format!("{}/{name}", folder.path());
            let seed = path.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
            sums.push_str(&format!(
                "{seed:016x}{:016x}  {path}\n",
                seed.rotate_left(17)
            ));
        }
    }
    shape.raw(&format!("{PROJECT}/checksums.md5"), sums.as_bytes());
}

pub fn shape() -> Shape {
    let mut shape = Shape::default();
    for (k, folder) in scanned() {
        for name in pages(k, folder) {
            shape.object(&format!("{}/{name}", folder.path()), kind::TIFF);
        }
        if folder.reference() == THUMBS.0 {
            shape.raw(&format!("{}/Thumbs.db", folder.path()), THUMBS.1);
        }
    }
    for path in map_files() {
        shape.object(&path, kind::TIFF);
    }
    project_files(&mut shape);
    shape.raw(LIST, &container_list());
    shape
}

/// The slots `expected()` names values for.
pub const SLOTS: [&str; 2] = ["dcterms:date", "dcterms:extent"];

/// One item per folder on disk and one for the oversize map, identified by
/// the reference as the folder or file name spells it, its files in page
/// order, its title, date and extent from its list row, and in the one set
/// its `Series` cell names. The unscanned folder's row describes nothing.
/// Left out: the scanner's `Thumbs.db`, the scanning project's files, and
/// the list itself.
pub fn expected() -> Outcome {
    let mut outcome = Outcome::default();
    let mut add = |folder: &Folder, media: Vec<String>, extent: String| {
        let mut item = Item::new(folder.title, &media, &[folder.series]);
        item.value("dcterms:date", folder.dates)
            .value("dcterms:extent", &extent);
        outcome.items.insert(folder.reference(), item);
    };
    for (k, folder) in scanned() {
        let media = pages(k, folder)
            .into_iter()
            .map(|name| format!("{}/{name}", folder.path()))
            .collect();
        add(folder, media, format!("{} leaves", leaves(k)));
    }
    add(&MAP, map_files(), "2 sheets".to_string());
    let thumbs = SCANNED
        .iter()
        .find(|folder| folder.reference() == THUMBS.0)
        .unwrap();
    outcome.left_out.extend([
        format!("{}/Thumbs.db", thumbs.path()),
        format!("{PROJECT}/scan-log.txt"),
        format!("{PROJECT}/checksums.md5"),
        LIST.to_string(),
    ]);
    outcome
}
