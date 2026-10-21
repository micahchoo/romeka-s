//! The outcome text of a derive, and its comparison with a pinned file.
//!
//! The text reads the model's JSON, the shape both the derive and the
//! preview route write, so the server's shape pins include this file and
//! write the same text. Record ids never appear: a record is named by its
//! ledger key, so a change in record order is no change of outcome.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;

/// The ledger's record key (`model::Origin::key`), read from the origin's
/// JSON.
pub fn ledger_key(origin: &Value) -> String {
    let file = |file: &Value| format!("{}/{}", text(&file["source"]), text(&file["path"]));
    if let Some(group) = origin.get("group") {
        match group.get("pattern") {
            Some(key) => format!("group:{}", text(key)),
            None => format!("own:{}", file(&group["own"])),
        }
    } else if let Some(found) = origin.get("file") {
        format!("file:{}", file(found))
    } else {
        let row = &origin["row"];
        format!("row:{}/{}", text(&row["sheet"]), text(&row["key"]))
    }
}

fn text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

/// An enum's JSON as words: `{"left":{"flag":"not-media"}}` is
/// `left flag not-media`.
fn words(value: &Value) -> String {
    match value {
        Value::Null => "none".into(),
        Value::String(text) => text.clone(),
        Value::Object(map) if map.len() == 1 => map
            .iter()
            .map(|(key, inner)| format!("{key} {}", words(inner)))
            .collect(),
        other => other.to_string(),
    }
}

/// The outcome of `records` (every record, any kind, as JSON), the import's
/// `new_sets` and its flags as `(kind, policy, count)`.
pub fn outcome(records: &[Value], new_sets: &[Value], flags: &[(Value, Value, usize)]) -> String {
    let keys: BTreeMap<u64, String> = records
        .iter()
        .filter_map(|record| Some((record["id"].as_u64()?, ledger_key(&record["origin"]))))
        .collect();
    let by_id = |id: &Value| {
        id.as_u64()
            .and_then(|id| keys.get(&id).cloned())
            .unwrap_or_else(|| format!("unknown record {id}"))
    };
    // A reference to another record names it by key; anything else as words.
    let reference = |value: &Value| match value.get("record") {
        Some(id) => by_id(id),
        None => words(value),
    };
    let mut ordered: Vec<(&str, String, &Value)> = records
        .iter()
        .map(|record| {
            let kind = record["kind"].as_str().unwrap_or("?");
            (kind, ledger_key(&record["origin"]), record)
        })
        .collect();
    ordered.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

    let mut out = String::new();
    for (kind, key, record) in ordered {
        let _ = writeln!(out, "{kind} {key} {}", words(&record["fate"]));
        let mut line = |label: &str, text: String| {
            let _ = writeln!(out, "  {label} {text}");
        };
        for file in record["files"].as_array().into_iter().flatten() {
            line(
                "file",
                format!("{}/{}", text(&file["source"]), text(&file["path"])),
            );
        }
        for row in record["rows"].as_array().into_iter().flatten() {
            line(
                "row",
                format!("{}/{}", text(&row["sheet"]), text(&row["key"])),
            );
        }
        if !record["parent"].is_null() {
            line("parent", reference(&record["parent"]));
        }
        for media in record["media"].as_array().into_iter().flatten() {
            line("media", by_id(media));
        }
        for (slot, values) in record["slots"].as_object().into_iter().flatten() {
            for value in values.as_array().into_iter().flatten() {
                let mut said = format!("{} {}", words(&value["type"]), value["text"]);
                if let Some(lang) = value["lang"].as_str() {
                    let _ = write!(said, " @{lang}");
                }
                if !value["label"].is_null() {
                    let _ = write!(said, " label {}", value["label"]);
                }
                if value["isPublic"] == Value::Bool(false) {
                    said.push_str(" private");
                }
                if !value["link"].is_null() {
                    let _ = write!(said, " -> {}", reference(&value["link"]));
                }
                line("slot", format!("{slot} {said}"));
            }
        }
        let resolved = &record["resolved"];
        for set in resolved["sets"].as_array().into_iter().flatten() {
            let named = set
                .get("new")
                .and_then(Value::as_u64)
                .and_then(|at| new_sets.get(usize::try_from(at).ok()?))
                .map(|new| format!("new {}", new["title"]));
            line("set", named.unwrap_or_else(|| reference(set)));
        }
        for (name, value) in resolved.as_object().into_iter().flatten() {
            let empty = value.is_null() || value.as_array().is_some_and(Vec::is_empty);
            if name != "sets" && !empty {
                line(name, words(value));
            }
        }
        if record["matches"]
            .as_array()
            .is_some_and(|ids| !ids.is_empty())
        {
            line("matches", record["matches"].to_string());
        }
    }
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (kind, policy, count) in flags {
        let policy = if policy.is_null() {
            "blocking".to_string()
        } else {
            words(policy)
        };
        *counts.entry((words(kind), policy)).or_default() += count;
    }
    for ((kind, policy), count) in counts {
        let _ = writeln!(out, "flag {kind} {policy} {count}");
    }
    out
}

/// Compare `actual` with the pinned `<dir>/<name>.txt`; with `PIN_WRITE=1`,
/// write it instead. The failure names each record whose block moved.
pub fn assert_pinned(dir: &Path, name: &str, actual: &str) {
    let path = dir.join(format!("{name}.txt"));
    if std::env::var_os("PIN_WRITE").is_some() {
        if let Err(error) = std::fs::write(&path, actual) {
            panic!("writing {}: {error}", path.display());
        }
        return;
    }
    let pinned = std::fs::read_to_string(&path).unwrap_or_default();
    if pinned != actual {
        panic!(
            "{name}: the outcome moved from {} (PIN_WRITE=1 rewrites it)\n{}",
            path.display(),
            block_diff(&pinned, actual)
        );
    }
}

/// Blocks by their head line's first two words (kind and key, or `flag`
/// and kind).
fn blocks(text: &str) -> BTreeMap<String, Vec<&str>> {
    let mut blocks: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut current = String::new();
    for line in text.lines() {
        if !line.starts_with(' ') {
            current = line.split(' ').take(2).collect::<Vec<_>>().join(" ");
        }
        blocks.entry(current.clone()).or_default().push(line);
    }
    blocks
}

fn block_diff(pinned: &str, actual: &str) -> String {
    const SHOWN: usize = 20;
    let (before, after) = (blocks(pinned), blocks(actual));
    let mut heads: Vec<&String> = before.keys().chain(after.keys()).collect();
    heads.sort();
    heads.dedup();
    let moved: Vec<&String> = heads
        .into_iter()
        .filter(|head| before.get(*head) != after.get(*head))
        .collect();
    let mut out = format!("{} blocks differ\n", moved.len());
    for head in moved.iter().take(SHOWN) {
        let old = before.get(*head).map(Vec::as_slice).unwrap_or_default();
        let new = after.get(*head).map(Vec::as_slice).unwrap_or_default();
        for line in old.iter().filter(|line| !new.contains(line)) {
            let _ = writeln!(out, "- {line}");
        }
        for line in new.iter().filter(|line| !old.contains(line)) {
            let _ = writeln!(out, "+ {line}");
        }
        if old.first() == new.first() {
            let _ = writeln!(out, "  (in {})", head);
        }
    }
    if moved.len() > SHOWN {
        let _ = writeln!(out, "... and {} more", moved.len() - SHOWN);
    }
    out
}
