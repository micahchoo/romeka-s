//! What the proposal says of each edit of the plan.

#![allow(clippy::expect_used)]

use super::NotApplied;
use super::bhc006_tests::proposal_with_lists;
use crate::import::plan::{CutEdit, Edit, FolderScope, ListFile, RelPath, SourceId};

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("a folder"),
    }
}

fn file(path: &str) -> ListFile {
    ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse(path).expect("a path"),
    }
}

#[test]
fn an_edit_on_a_folder_that_is_gone_says_so() {
    let list = |path: &str| Edit::List {
        scope: scope("Artifacts"),
        list: Some(file(path)),
    };
    let edits = [
        Edit::LeaveOut {
            scope: scope("no such folder"),
        },
        Edit::Grain {
            list: file("no such list.xlsx"),
            describes: crate::import::plan::RecordKind::Media,
        },
        // No count makes items of the first two places of these names.
        Edit::Items {
            scope: scope("Artifacts"),
            cut: CutEdit::Places { places: 2 },
        },
        list("Artifacts/BHC006_GMT_AF.csv"),
        list("BHC006_GMT_AF.xlsx"),
    ];
    let proposal = proposal_with_lists(&edits);
    let said: Vec<(bool, Option<NotApplied>)> = proposal
        .edits
        .iter()
        .map(|edit| (edit.applied, edit.reason))
        .collect();
    assert_eq!(
        said,
        [
            (false, Some(NotApplied::Gone)),
            (false, Some(NotApplied::Gone)),
            (false, Some(NotApplied::NotOffered)),
            (false, Some(NotApplied::Replaced)),
            (true, None),
        ]
    );
}
