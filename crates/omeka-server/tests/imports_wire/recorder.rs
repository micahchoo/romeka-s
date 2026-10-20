//! Writing and checking the fixture files: the folder, the update switch, and the
//! normal form of an answer (sorted keys, steady times).

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::{Map, Value, json};

/// The times and the countdown change from run to run.
pub const VOLATILE_TIMES: [&str; 4] = ["created", "finished", "scanned", "died"];

pub struct Fixtures {
    dir: PathBuf,
    update: bool,
    seen: BTreeSet<String>,
    differ: Vec<String>,
}

impl Fixtures {
    pub fn new() -> Self {
        Self {
            dir: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../web/admin/tests/fixtures/import"),
            update: std::env::var_os("UPDATE_FIXTURES").is_some(),
            seen: BTreeSet::new(),
            differ: Vec::new(),
        }
    }

    pub fn text(&mut self, name: &str, text: &str) {
        let path = self.dir.join(name);
        self.seen.insert(name.to_string());
        if self.update {
            std::fs::create_dir_all(&self.dir).unwrap();
            std::fs::write(&path, text).unwrap();
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(text) {
            self.differ.push(name.to_string());
        }
    }

    pub fn json(&mut self, name: &str, value: &Value) {
        let mut value = value.clone();
        steady(&mut value);
        let text = serde_json::to_string_pretty(&sorted(value)).unwrap() + "\n";
        self.text(name, &text);
    }

    /// A committed input the test reads; it counts as produced, so it is not stale.
    pub fn read(&mut self, name: &str) -> Value {
        self.seen.insert(name.to_string());
        let text = std::fs::read_to_string(self.dir.join(name)).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    pub fn finish(self) {
        let found: BTreeSet<String> = std::fs::read_dir(&self.dir)
            .map(|listing| {
                listing
                    .flatten()
                    .map(|file| file.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let stale: Vec<&String> = found.difference(&self.seen).collect();
        if self.update {
            for name in stale {
                std::fs::remove_file(self.dir.join(name)).unwrap();
            }
            return;
        }
        assert!(
            self.differ.is_empty() && stale.is_empty(),
            "fixtures out of date: differ {:?}, not produced {stale:?}; run with UPDATE_FIXTURES=1",
            self.differ
        );
    }
}

pub fn sorted(value: Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

pub fn steady(value: &mut Value) {
    match value {
        Value::Object(members) => {
            for (key, member) in members.iter_mut() {
                if VOLATILE_TIMES.contains(&key.as_str()) && member.is_string() {
                    *member = json!("2026-10-21T09:00:00+00:00");
                } else if key == "secondsLeft" && member.is_number() {
                    *member = json!(42);
                } else {
                    steady(member);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(steady),
        _ => {}
    }
}
