//! Frozen held-out shape, written by the keeper before the proposer was built: never edit it or `expected()`.
//!
//! The Marlow Valley Museum: a small town museum's accession register and
//! its object photographs. The register has one row per accessioned object,
//! numbered year, lot and object (`2019.4.1`; numbers from before 2000 have
//! a two-digit year). An object in parts has the part letters on its number
//! (`2019.4.2a-b`), and its photographs name the whole object or one part.
//! The objects were photographed in sessions, one folder each, with the view
//! number after the accession number. The 2021 reshoot photographed some
//! objects again, under the same file names. Objects accessioned after the
//! register was exported are photographed and have no row. The register
//! holds objects never photographed, and one object the museum has since
//! deaccessioned. Condition reports are kept beside the photographs.

use super::{Item, Outcome, Shape, kind};

/// The keeper's v3 plan of this archive: proof that the outcome is reachable.
pub const PLAN: &str = include_str!("fixtures/accession_register.json");

/// One row of the register.
struct Object {
    number: &'static str,
    name: &'static str,
    description: &'static str,
    maker: &'static str,
    made: &'static str,
    materials: &'static str,
    classification: &'static str,
    credit: &'static str,
    location: &'static str,
    status: &'static str,
}

#[expect(clippy::too_many_arguments, reason = "one register row, cell by cell")]
const fn object(
    number: &'static str,
    name: &'static str,
    description: &'static str,
    maker: &'static str,
    made: &'static str,
    materials: &'static str,
    classification: &'static str,
    credit: &'static str,
    location: &'static str,
    status: &'static str,
) -> Object {
    Object {
        number,
        name,
        description,
        maker,
        made,
        materials,
        classification,
        credit,
        location,
        status,
    }
}

const POLLARD: &str = "Gift of Mr J. Pollard";
const ELLIS: &str = "Gift of the Ellis family";
const HARTLEY: &str = "Gift of Mrs E. Hartley";
const RAILWAY: &str = "Transferred from the Marlow Valley Railway Society";
const SINGH: &str = "Gift of Mr R. Singh";
const INSTITUTE: &str = "Gift of the Marlow Women's Institute";

/// The register, in its own order.
const REGISTER: [Object; 19] = [
    object(
        "78.12.3",
        "Butter churn",
        "Wooden plunger churn with iron bands",
        "",
        "c. 1900",
        "oak, iron",
        "Domestic",
        POLLARD,
        "Store B, bay 2",
        "",
    ),
    object(
        "78.12.4",
        "Flat iron",
        "Sad iron with a cast handle",
        "Kenrick",
        "c. 1890",
        "cast iron",
        "Domestic",
        POLLARD,
        "Store B, bay 2",
        "",
    ),
    object(
        "85.3.1",
        "Shepherd's crook",
        "Hazel crook with a carved horn head",
        "",
        "1920s",
        "hazel, horn",
        "Agriculture",
        ELLIS,
        "Store A, bay 1",
        "",
    ),
    object(
        "85.3.2",
        "Sheep shears",
        "Hand shears, one blade broken",
        "Burgon & Ball",
        "c. 1930",
        "steel",
        "Agriculture",
        ELLIS,
        "",
        "Deaccessioned 2016",
    ),
    object(
        "2019.4.1",
        "Christening gown",
        "Cotton lawn christening gown with lace insertions",
        "",
        "c. 1885",
        "cotton, lace",
        "Costume",
        HARTLEY,
        "Textile store, drawer 4",
        "",
    ),
    object(
        "2019.4.2a-b",
        "Teapot and lid",
        "Brown glazed earthenware teapot with its lid",
        "",
        "c. 1910",
        "earthenware",
        "Domestic",
        HARTLEY,
        "Store B, bay 4",
        "",
    ),
    object(
        "2019.4.3",
        "Sampler",
        "Alphabet sampler worked by Ann Hartley aged 11",
        "Ann Hartley",
        "1842",
        "linen, silk",
        "Costume",
        HARTLEY,
        "Textile store, drawer 1",
        "",
    ),
    object(
        "2019.4.4",
        "Photograph album",
        "Family album of cabinet cards and snapshots",
        "",
        "1900-1915",
        "leather, card",
        "Photographs",
        HARTLEY,
        "Paper store, shelf 3",
        "",
    ),
    object(
        "2019.4.5",
        "Hat pin",
        "Hat pin with a blue glass head",
        "",
        "c. 1905",
        "steel, glass",
        "Costume",
        HARTLEY,
        "Textile store, drawer 4",
        "",
    ),
    object(
        "2019.11.1",
        "Hand lamp",
        "Railway hand lamp from Marlow Valley station",
        "",
        "c. 1950",
        "steel, glass",
        "Transport",
        RAILWAY,
        "Store C",
        "",
    ),
    object(
        "2019.11.2",
        "Ticket punch",
        "Ticket nipper used by the station staff",
        "",
        "",
        "steel",
        "Transport",
        RAILWAY,
        "Store C",
        "",
    ),
    object(
        "2019.11.3",
        "Station clock",
        "Wall clock from the booking office",
        "Smith & Sons",
        "1938",
        "oak, brass",
        "Transport",
        RAILWAY,
        "Gallery 2",
        "",
    ),
    object(
        "2020.2.1",
        "Ration book",
        "Ration book issued to the Singh household",
        "Ministry of Food",
        "1943",
        "paper",
        "Documents",
        SINGH,
        "Paper store, shelf 1",
        "",
    ),
    object(
        "2020.2.2",
        "Gas mask and case",
        "Civilian respirator in its card box",
        "",
        "1939",
        "rubber, card",
        "",
        SINGH,
        "Store B, bay 6",
        "",
    ),
    object(
        "2020.2.3",
        "Identity card",
        "National Registration identity card",
        "",
        "1940",
        "card",
        "Documents",
        SINGH,
        "Paper store, shelf 1",
        "",
    ),
    object(
        "2021.1.1",
        "Toy horse",
        "Wooden horse on wheels, painted",
        "",
        "c. 1920",
        "wood, paint",
        "Toys",
        INSTITUTE,
        "Store B, bay 1",
        "",
    ),
    object(
        "2021.1.2a-b",
        "Doll and cradle",
        "Composition doll in a rocking cradle",
        "",
        "c. 1935",
        "composition, wood",
        "Toys",
        INSTITUTE,
        "Store B, bay 1",
        "",
    ),
    // The cataloguer typed this classification in lower case.
    object(
        "2021.1.3",
        "Spinning top",
        "Turned top with its whip",
        "",
        "",
        "wood",
        "toys",
        INSTITUTE,
        "Store B, bay 1",
        "",
    ),
    object(
        "2022.5.1",
        "Milk bottle",
        "Pint bottle of the Marlow Valley Dairy",
        "Marlow Valley Dairy",
        "c. 1960",
        "glass",
        "Domestic",
        "Found in the collection, 2022",
        "Store B, bay 3",
        "",
    ),
];

const DEACCESSIONED: &str = "85.3.2";

/// The photographs, by the object whose record shows them, in session
/// order, then by name. The 2019 camera wrote `.JPG`; the reshoot's export
/// wrote `.jpg`.
const PHOTOS: [(&str, &[&str]); 16] = [
    (
        "78.12.3",
        &[
            "Photography/2019-06 studio/78.12.3_01.JPG",
            "Photography/2019-06 studio/78.12.3_02.JPG",
        ],
    ),
    ("85.3.1", &["Photography/2019-06 studio/85.3.1_01.JPG"]),
    (
        "2019.4.1",
        &[
            "Photography/2019-06 studio/2019.4.1_01.JPG",
            "Photography/2019-06 studio/2019.4.1_02.JPG",
            "Photography/2019-06 studio/2019.4.1_03.JPG",
            "Photography/2021-02 reshoot/2019.4.1_01.jpg",
        ],
    ),
    (
        "2019.4.2a-b",
        &[
            "Photography/2019-06 studio/2019.4.2a-b_01.JPG",
            "Photography/2019-06 studio/2019.4.2a_01.JPG",
            "Photography/2019-06 studio/2019.4.2b_01.JPG",
        ],
    ),
    (
        "2019.4.3",
        &[
            "Photography/2019-06 studio/2019.4.3_01.JPG",
            "Photography/2021-02 reshoot/2019.4.3_01.jpg",
            "Photography/2021-02 reshoot/2019.4.3_02.jpg",
        ],
    ),
    // The photographer padded the lot number.
    ("2019.4.5", &["Photography/2019-06 studio/2019.04.5_01.JPG"]),
    (
        "2019.11.1",
        &[
            "Photography/2021-02 reshoot/2019.11.1_01.jpg",
            "Photography/2021-02 reshoot/2019.11.1_02.jpg",
        ],
    ),
    (
        "2019.11.3",
        &["Photography/2021-02 reshoot/2019.11.3_01.jpg"],
    ),
    (
        "2020.2.1",
        &[
            "Photography/2021-02 reshoot/2020.2.1_01.jpg",
            "Photography/2021-02 reshoot/2020.2.1_02.jpg",
        ],
    ),
    ("2020.2.3", &["Photography/2021-02 reshoot/2020.2.3_01.jpg"]),
    // The second view's name has a capital O for the zero.
    (
        "2021.1.1",
        &[
            "Photography/2021-02 reshoot/2021.1.1_01.jpg",
            "Photography/2021-02 reshoot/2O21.1.1_02.jpg",
        ],
    ),
    (
        "2021.1.2a-b",
        &[
            "Photography/2021-02 reshoot/2021.1.2a-b_01.jpg",
            "Photography/2021-02 reshoot/2021.1.2a_01.jpg",
            "Photography/2021-02 reshoot/2021.1.2b_01.jpg",
        ],
    ),
    (
        "2021.1.3",
        &[
            "Photography/2021-02 reshoot/2021.1.3_01.jpg",
            "Photography/2021-02 reshoot/2021.1.3_02.jpg",
        ],
    ),
    (
        "2023.1.1",
        &[
            "Photography/2023 new accessions/2023.1.1_01.jpg",
            "Photography/2023 new accessions/2023.1.1_02.jpg",
        ],
    ),
    (
        "2023.1.2",
        &["Photography/2023 new accessions/2023.1.2_01.jpg"],
    ),
    (DEACCESSIONED, &["Photography/2019-06 studio/85.3.2_01.JPG"]),
];

/// Files that show no object or are the museum's own working papers.
const CONDITION_REPORTS: [&str; 3] = [
    "Condition reports/2019.4.1 condition report.pdf",
    "Condition reports/2019.4.3 condition report.pdf",
    "Condition reports/78.12.3 condition 2015.docx",
];
const COLOUR_TARGET: &str = "Photography/2019-06 studio/colour checker.JPG";
const SYSTEM: [(&str, &[u8]); 2] = [
    (
        "Photography/2019-06 studio/Thumbs.db",
        b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00",
    ),
    (
        "Photography/2021-02 reshoot/.DS_Store",
        b"\x00\x00\x00\x01Bud1",
    ),
];
const REGISTER_FILE: &str = "Collections register.xlsx";

/// The register as the collections system exported it: a title line, then
/// the header on line 2.
fn register() -> Vec<u8> {
    let mut book = rust_xlsxwriter::Workbook::new();
    let page = book.add_worksheet();
    page.set_name("Register").unwrap();
    page.write(
        0,
        0,
        "Marlow Valley Museum: accession register (exported 14/02/2023)",
    )
    .unwrap();
    let header = [
        "Accession No.",
        "Object name",
        "Description",
        "Maker",
        "Date made",
        "Materials",
        "Classification",
        "Credit line",
        "Location",
        "Status",
    ];
    for (col, text) in (0_u16..).zip(header) {
        page.write(1, col, text).unwrap();
    }
    for (line, row) in (2_u32..).zip(&REGISTER) {
        let cells = [
            row.number,
            row.name,
            row.description,
            row.maker,
            row.made,
            row.materials,
            row.classification,
            row.credit,
            row.location,
            row.status,
        ];
        for (col, text) in (0_u16..).zip(cells) {
            if !text.is_empty() {
                page.write(line, col, text).unwrap();
            }
        }
    }
    book.save_to_buffer().unwrap()
}

pub fn shape() -> Shape {
    let mut shape = Shape::default();
    for (_, paths) in PHOTOS {
        for path in paths {
            shape.object(path, kind::JPEG);
        }
    }
    shape.object(COLOUR_TARGET, kind::JPEG);
    for (path, bytes) in SYSTEM {
        shape.raw(path, bytes);
    }
    for path in CONDITION_REPORTS {
        let header = if path.ends_with(".pdf") {
            kind::PDF
        } else {
            kind::ZIP
        };
        shape.object(path, header);
    }
    shape.raw(REGISTER_FILE, &register());
    shape
}

/// The slots `expected()` names values for.
pub const SLOTS: [&str; 5] = [
    "dcterms:description",
    "dcterms:creator",
    "dcterms:date",
    "dcterms:medium",
    "dcterms:provenance",
];

/// One item per object the museum holds, identified by its accession number
/// as the register spells it, parts letters and all. Its photographs are its
/// media, the whole object and its parts alike, in session order and then
/// by view. Its register row gives its title, description, maker, date,
/// materials and credit line, and its classification names its one set.
/// An object in the register with no photograph is still an item: the
/// register is the museum's catalogue. An object photographed after the
/// export has no row, so it is titled by its number and is in no set. The
/// deaccessioned object makes no item, and its photograph is left out with
/// it. Left out too: the colour target, the system files, the condition
/// reports and the register itself.
pub fn expected() -> Outcome {
    let mut outcome = Outcome::default();
    let photos_of = |number: &str| -> Vec<String> {
        PHOTOS
            .iter()
            .find(|(of, _)| *of == number)
            .map(|(_, paths)| paths.iter().map(|path| path.to_string()).collect())
            .unwrap_or_default()
    };
    for row in REGISTER.iter().filter(|row| row.number != DEACCESSIONED) {
        let set = match row.classification {
            "" => vec![],
            "toys" => vec!["Toys"],
            set => vec![set],
        };
        let mut item = Item::new(row.name, &photos_of(row.number), &set);
        for (slot, text) in [
            ("dcterms:description", row.description),
            ("dcterms:creator", row.maker),
            ("dcterms:date", row.made),
            ("dcterms:medium", row.materials),
            ("dcterms:provenance", row.credit),
        ] {
            if !text.is_empty() {
                item.value(slot, text);
            }
        }
        outcome.items.insert(row.number.to_string(), item);
    }
    for number in ["2023.1.1", "2023.1.2"] {
        outcome.items.insert(
            number.to_string(),
            Item::new(number, &photos_of(number), &[]),
        );
    }
    outcome.left_out.extend(photos_of(DEACCESSIONED));
    outcome.left_out.insert(COLOUR_TARGET.to_string());
    outcome
        .left_out
        .extend(SYSTEM.iter().map(|(path, _)| path.to_string()));
    outcome
        .left_out
        .extend(CONDITION_REPORTS.iter().map(|path| path.to_string()));
    outcome.left_out.insert(REGISTER_FILE.to_string());
    outcome
}
