//! What the person made that the scan cannot see: the lists they uploaded
//! and their free sets. A proposal keeps them, so taking its structure loses
//! nothing.

#![allow(clippy::expect_used)]

use serde_json::json;

use super::{Evidence, KeptList, NodeEvidence, Proposal, Share, propose};
use crate::import::derive::Sheets;
use crate::import::derive::tests::support::{JPEG, catalog, index, plan};
use crate::import::plan::{
    Edit, FolderScope, ImportPlan, RecordKind, RelPath, SetByTitle, SetSource, SheetId, SlotName,
    SourceId,
};
use crate::import::sheet::{self, SheetError};

/// A staged list as the upload writes it: joined on its first column.
fn staged(id: &str, headers: &[&str]) -> serde_json::Value {
    let columns: Vec<serde_json::Value> = headers
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, header)| {
            json!({
                "column": { "index": index, "header": header },
                "options": { "split": "|", "type": "literal", "lang": null, "private": false }
            })
        })
        .collect();
    json!({
        "id": id, "source": "staging", "file": format!("lists/{id}.csv"), "hash": null,
        "read": { "encoding": "auto", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": true },
        "rowKind": "item",
        "join": {
            "on": "identifier",
            "column": { "index": 0, "header": headers[0] },
            "normalize": { "caseFold": false, "leadingZeros": false, "separators": false }
        },
        "itemKey": null,
        "columns": columns
    })
}

/// The person's plan of one series: `register` uploaded with a title and a
/// sets column, `gone` uploaded and no longer readable, and a set by title.
fn person() -> ImportPlan {
    let headers = ["Identifier", "Title [dcterms:title]", "Item sets [sets]"];
    let column = |index: usize| json!({ "from": "column", "sheet": "register", "column": { "index": index, "header": headers[index] } });
    let mut person = plan(|plan| {
        plan["sources"]
            .as_array_mut()
            .expect("sources")
            .push(json!({ "kind": "staging", "id": "staging" }));
        plan["sheets"] = json!([
            staged("register", &headers),
            staged("gone", &["Identifier"])
        ]);
        plan["slots"]["item"]["dcterms:title"] =
            json!({ "sources": [column(1)], "combine": "first" });
        plan["slots"]["item"]["sets"] = json!({ "sources": [column(2)], "combine": "first" });
    });
    person.structure.push_free_set(SetSource::Title(SetByTitle {
        title: "Donated".into(),
        description: None,
    }));
    person
}

fn kept_on(proposal: &Proposal, series: &str) -> Vec<KeptList> {
    proposal
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Series(node) if node.level.0 == series => Some(node.kept.clone()),
            _ => None,
        })
        .expect("the series node")
}

#[test]
fn a_proposal_keeps_the_uploaded_lists_and_the_free_sets() {
    let person = person();
    let index = index(&[
        ("Letters/ana.jpg", JPEG),
        ("Letters/ben.jpg", JPEG),
        ("Maps/harbour.jpg", JPEG),
    ]);
    let read = |id: &str| {
        let (_, list) = person
            .structure
            .list(&SheetId(id.into()))
            .expect("a staged list");
        list.read.clone()
    };
    let mut sheets = Sheets::new();
    sheets.insert(
        SheetId("register".into()),
        sheet::read(
            "Identifier,Title [dcterms:title],Item sets [sets]\nharbour,Harbour,2018\nnowhere,Lost,2018\n"
                .as_bytes(),
            &read("register"),
            None,
        ),
    );
    sheets.insert(
        SheetId("gone".into()),
        Err(SheetError::Io("not in staging".into())),
    );
    let catalog = catalog();
    let evidence = Evidence {
        index: &index,
        lists: &Vec::new(),
        catalog: &catalog,
        sources: &person.sources,
        structure: &person.structure,
        sheets: &sheets,
    };
    let proposal = propose(
        &evidence,
        &[Edit::LeaveOut {
            scope: FolderScope {
                source: SourceId("s1".into()),
                folder: RelPath::parse("Nothing here").expect("a folder"),
            },
        }],
    );

    let held = |series: &str| -> Vec<String> {
        proposal
            .structure
            .all_series()
            .find(|(_, one)| one.id().0 == series)
            .map(|(_, one)| one.lists().iter().map(|list| list.id.0.clone()).collect())
            .expect("the series")
    };
    assert_eq!(held("Maps"), ["register"], "the register pairs a map");
    assert_eq!(
        held("Letters"),
        ["gone"],
        "a list that pairs nowhere goes first"
    );
    let (maps, register) = proposal
        .structure
        .list(&SheetId("register".into()))
        .expect("kept");
    assert_eq!(
        Some(register),
        person
            .structure
            .list(&SheetId("register".into()))
            .map(|(_, list)| list),
        "as the person sent it"
    );
    assert!(
        proposal
            .structure
            .series(maps)
            .rules(RecordKind::Item)
            .contains_key(&SlotName::parse("dcterms:title").expect("a term")),
        "where its title column goes"
    );
    assert_eq!(
        proposal.structure.free_sets(),
        person.structure.free_sets(),
        "the sets column of the list, and the set by title"
    );
    assert_eq!(
        kept_on(&proposal, "Maps")
            .iter()
            .map(|kept| (kept.list.0.as_str(), kept.rows))
            .collect::<Vec<_>>(),
        [("register", Some(Share { n: 1, of: 2 }))],
        "one of its two rows names an item here"
    );
    assert_eq!(
        kept_on(&proposal, "Letters")
            .iter()
            .map(|kept| (kept.list.0.as_str(), kept.rows))
            .collect::<Vec<_>>(),
        [("gone", None)],
        "a list that cannot be read is kept, and says so"
    );
}
