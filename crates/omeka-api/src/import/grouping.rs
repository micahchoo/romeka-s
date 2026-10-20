//! Files into items and folders into item sets.
//!
//! The pattern's `identifier` group makes files with the same text one item.
//! The plan's set level turns each file's folders into set keys; a set is on
//! when the plan says so, or by default when any of its files matched. An item
//! whose every set is off is left out. An item from files at the top of the
//! import folder has no set keys, and stays.

use std::collections::BTreeMap;

use serde::Serialize;

use super::archive::ArchiveFile;
use super::catalog::Catalog;
use super::pattern::{self, FilePattern, Groups};
use super::plan::{ImportPlan, SetLevel};

/// One file of the folder with what the pattern read from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedFile {
    pub file: ArchiveFile,
    /// `None` when the pattern did not match.
    pub groups: Option<Groups>,
    pub set_keys: Vec<String>,
    /// The index of its item in [`Grouping::items`].
    pub item: Option<usize>,
}

impl MatchedFile {
    #[must_use]
    pub fn group(&self, name: &str) -> Option<&str> {
        self.groups
            .as_ref()?
            .get(name)
            .map(|group| group.text.as_str())
    }
}

/// An item the files make, before the spreadsheet joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileItem {
    pub identifier: String,
    /// Indices into [`Grouping::files`], in path order.
    pub files: Vec<usize>,
    /// The keys of its sets that are on.
    pub set_keys: Vec<String>,
    /// False when every one of its sets is off.
    pub kept: bool,
}

/// One candidate item set, as the Item sets tab shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetEntry {
    pub key: String,
    pub name: String,
    #[serde(rename = "use")]
    pub use_set: bool,
    pub matched: usize,
    pub total: usize,
    /// The items whose first set is this one, so whose type it decides.
    pub items: usize,
    pub existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grouping {
    pub files: Vec<MatchedFile>,
    pub sets: Vec<SetEntry>,
    pub items: Vec<FileItem>,
}

/// The set keys of a file at `path` under `level`.
fn set_keys(level: SetLevel, path: &str, groups: Option<&Groups>) -> Vec<String> {
    let mut dirs: Vec<&str> = path.split('/').collect();
    dirs.pop();
    let first = dirs.first().map(|dir| (*dir).to_string());
    let keys: Vec<String> = match level {
        SetLevel::Level1 => first.into_iter().collect(),
        SetLevel::Level2 => dirs
            .get(1)
            .map(|dir| (*dir).to_string())
            .or(first)
            .into_iter()
            .collect(),
        SetLevel::Both => dirs.iter().take(2).map(|dir| (*dir).to_string()).collect(),
        SetLevel::Path => (!dirs.is_empty())
            .then(|| dirs.join(" / "))
            .into_iter()
            .collect(),
        SetLevel::Group => groups
            .and_then(|groups| groups.get(pattern::SET))
            .map(|group| group.text.clone())
            .filter(|text| !text.is_empty())
            .or(first)
            .into_iter()
            .collect(),
    };
    keys
}

/// Match every file and gather the sets and items.
#[must_use]
pub fn group(
    plan: &ImportPlan,
    files: &[ArchiveFile],
    pattern: Option<&FilePattern>,
    catalog: &Catalog,
) -> Grouping {
    let mut matched: Vec<MatchedFile> = files
        .iter()
        .map(|file| {
            let groups = pattern.and_then(|pattern| pattern.groups(&file.path));
            MatchedFile {
                set_keys: set_keys(plan.set_level, &file.path, groups.as_ref()),
                file: file.clone(),
                groups,
                item: None,
            }
        })
        .collect();
    let mut sets = sets(plan, &matched, catalog);
    let on: Vec<&str> = sets
        .iter()
        .filter(|set| set.use_set)
        .map(|set| set.key.as_str())
        .collect();

    let mut items: Vec<FileItem> = Vec::new();
    let mut by_identifier: BTreeMap<String, usize> = BTreeMap::new();
    for (index, file) in matched.iter_mut().enumerate() {
        let Some(identifier) = file.group(pattern::IDENTIFIER).map(str::to_string) else {
            continue;
        };
        let at = *by_identifier.entry(identifier.clone()).or_insert_with(|| {
            items.push(FileItem {
                identifier,
                files: Vec::new(),
                set_keys: Vec::new(),
                kept: true,
            });
            items.len() - 1
        });
        file.item = Some(at);
        items[at].files.push(index);
    }
    for item in &mut items {
        let mut all: Vec<&str> = Vec::new();
        for key in item
            .files
            .iter()
            .flat_map(|&index| &matched[index].set_keys)
        {
            if !all.contains(&key.as_str()) {
                all.push(key);
            }
        }
        item.set_keys = all
            .iter()
            .filter(|key| on.contains(key))
            .map(|key| (*key).to_string())
            .collect();
        item.kept = all.is_empty() || !item.set_keys.is_empty();
    }
    for set in &mut sets {
        set.items = items
            .iter()
            .filter(|item| item.set_keys.first() == Some(&set.key))
            .count();
    }
    Grouping {
        files: matched,
        sets,
        items,
    }
}

fn sets(plan: &ImportPlan, files: &[MatchedFile], catalog: &Catalog) -> Vec<SetEntry> {
    let mut sets: Vec<SetEntry> = Vec::new();
    for file in files {
        for key in &file.set_keys {
            let at = match sets.iter().position(|set| set.key == *key) {
                Some(at) => at,
                None => {
                    sets.push(SetEntry {
                        key: key.clone(),
                        name: key.clone(),
                        use_set: false,
                        matched: 0,
                        total: 0,
                        items: 0,
                        existing: false,
                    });
                    sets.len() - 1
                }
            };
            sets[at].total += 1;
            if file.groups.is_some() {
                sets[at].matched += 1;
            }
        }
    }
    for set in &mut sets {
        match plan.sets.get(&set.key) {
            Some(choice) => {
                set.use_set = choice.use_set;
                set.name.clone_from(&choice.name);
            }
            None => set.use_set = set.matched > 0,
        }
        set.existing = catalog.item_sets.contains_key(set.name.trim());
    }
    sets
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn groups_with_set(text: &str) -> Groups {
        Groups::from([(
            "set".to_string(),
            pattern::Group {
                text: text.to_string(),
                utf16: (0, 0),
            },
        )])
    }

    #[test]
    fn each_set_level_reads_its_folders() {
        let path = "Photographs/1950s/BHC_006_PH_101.tif";
        assert_eq!(set_keys(SetLevel::Level1, path, None), ["Photographs"]);
        assert_eq!(set_keys(SetLevel::Level2, path, None), ["1950s"]);
        assert_eq!(
            set_keys(SetLevel::Level2, "Artifacts/a.jpg", None),
            ["Artifacts"]
        );
        assert_eq!(
            set_keys(SetLevel::Both, path, None),
            ["Photographs", "1950s"]
        );
        assert_eq!(
            set_keys(SetLevel::Path, path, None),
            ["Photographs / 1950s"]
        );
        assert_eq!(
            set_keys(SetLevel::Group, path, Some(&groups_with_set("PH"))),
            ["PH"]
        );
        assert_eq!(set_keys(SetLevel::Group, path, None), ["Photographs"]);
        assert!(set_keys(SetLevel::Level1, "top.jpg", None).is_empty());
    }
}
