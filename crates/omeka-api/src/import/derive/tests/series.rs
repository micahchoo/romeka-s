//! A plan of several series: each takes the files under its folder, groups
//! them with its own pattern, and joins only its own lists.

use serde_json::{Value as Json, json};

use super::support::{
    BINARY, JPEG, TEXT, col, index, join_on, kept, media_paths, read_sheets, rule, run, sheet_plan,
    texts,
};
use crate::import::decisions::EntryRef;
use crate::import::model::{LeftBy, Record};
use crate::import::plan::{ImportPlan, RecordKind};

const BORN_DIGITAL: &str = include_str!("../../fixtures/v3/born_digital.json");

/// `<identifier>[_<part>].<ext>`, relative to the series' folder.
const NAME: &str = r"^(?<identifier>[^/_]+)(?:_(?<part>[^/.]+))?\.[^/.]+$";

/// A series over `folder` of `s1` whose items take their title from
/// column 1 of its one list, joined on column 0.
fn series(id: &str, folder: &str, list: &str) -> Json {
    let mut sheet = sheet_plan(list, &["Code", "Title"]);
    sheet["join"] = join_on("identifier", 0, "Code", false, false, false);
    json!({
        "kind": "series", "id": id, "set": null,
        "scope": [{ "source": "s1", "folder": folder }],
        "grouping": { "pattern": NAME, "caseFold": false },
        "example": null,
        "lists": [sheet],
        "slots": {
            "item": { "dcterms:title": rule(json!([col(list, 1, "Title")]), "first") },
            "media": {}, "itemSet": {}
        }
    })
}

fn plan_of(levels: Json) -> ImportPlan {
    let mut value: Json = serde_json::from_str(BORN_DIGITAL).expect("fixture json");
    value["structure"]["levels"] = levels;
    ImportPlan::from_json(&value.to_string()).unwrap_or_else(|error| panic!("{error}: {value}"))
}

fn by_key<'m>(items: &[&'m Record], key: &str) -> &'m Record {
    items
        .iter()
        .find(|record| record.origin.key() == key)
        .unwrap_or_else(|| panic!("no item {key}"))
}

#[test]
fn each_series_groups_and_joins_only_its_own_files() {
    let files = [
        ("Letters/A1_p1.jpg", JPEG),
        ("Letters/A1_p2.jpg", JPEG),
        ("Letters/B2.jpg", JPEG),
        ("Photos/A1.jpg", JPEG),
        ("Extras/A1.jpg", JPEG),
        ("notes.txt", TEXT),
        ("Photos/B2.bin", BINARY),
    ];
    let plan = plan_of(json!([
        series("letters", "Letters", "register"),
        series("photos", "Photos", "pictures"),
    ]));
    let sheets = read_sheets(
        &plan,
        &[
            ("register", b"Code,Title\nA1,Letter one\nB2,Letter two\n"),
            ("pictures", b"Code,Title\nA1,Photo one\n"),
        ],
    );
    let model = run(&index(&files), &sheets, &plan);
    let items = kept(&model, RecordKind::Item);

    let mut keys: Vec<String> = items.iter().map(|record| record.origin.key()).collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "group:letters/A1",
            "group:letters/B2",
            "group:photos/A1",
            "group:photos/B2"
        ],
        "two series make the key A1, and each record key names its series"
    );

    let letter = by_key(&items, "group:letters/A1");
    assert_eq!(texts(letter, "dcterms:title"), ["Letter one"]);
    assert_eq!(
        media_paths(&model, letter),
        ["Letters/A1_p1.jpg", "Letters/A1_p2.jpg"],
        "the pattern matched the name inside the series' folder"
    );
    let photo = by_key(&items, "group:photos/A1");
    assert_eq!(
        texts(photo, "dcterms:title"),
        ["Photo one"],
        "a list joins only its own series' groups"
    );
    assert_eq!(media_paths(&model, photo), ["Photos/A1.jpg"]);
    assert!(
        texts(by_key(&items, "group:photos/B2"), "dcterms:title").is_empty(),
        "the register's B2 row does not join the photos' B2"
    );

    for path in ["Extras/A1.jpg", "notes.txt"] {
        let state = model
            .entries
            .iter()
            .find(
                |state| matches!(&state.entry, EntryRef::File(file) if file.path.as_str() == path),
            )
            .unwrap_or_else(|| panic!("no entry {path}"));
        assert_eq!(state.record, None, "{path} makes no record");
        assert_eq!(state.left, Some(LeftBy::Outside), "{path} is outside");
        assert!(!state.unmatched, "{path} is outside, not unmatched");
    }
}

#[test]
fn a_plan_of_one_series_keeps_the_v2_record_key() {
    let plan = plan_of(json!([series("letters", "Letters", "register")]));
    let sheets = read_sheets(&plan, &[("register", b"Code,Title\nA1,Letter one\n")]);
    let model = run(&index(&[("Letters/A1.jpg", JPEG)]), &sheets, &plan);
    let keys: Vec<String> = kept(&model, RecordKind::Item)
        .iter()
        .map(|record| record.origin.key())
        .collect();
    assert_eq!(keys, ["group:A1"]);
}
