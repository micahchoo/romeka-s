//! The paths the shipped origin forwards to the process
//! (`deploy/Caddyfile`, the `@app` matcher) are the paths the guides tell an
//! operator to forward. An origin written from a guide that names fewer
//! breaks the routes it leaves out.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::Path;

fn read(relative: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(relative)).unwrap()
}

/// A route by its prefix: `/s`, `/s/` and `/s/*` are one route.
fn routes<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    paths
        .map(|path| path.trim_end_matches('*').trim_end_matches('/').to_string())
        .collect()
}

fn caddy_forwards() -> BTreeSet<String> {
    let caddyfile = read("deploy/Caddyfile");
    let line = caddyfile
        .lines()
        .find_map(|line| line.trim().strip_prefix("@app path "))
        .expect("the @app matcher");
    routes(line.split_whitespace())
}

/// The backticked paths of the sentence that starts at `forwards` in `doc`.
fn guide_forwards(doc: &str) -> BTreeSet<String> {
    let text = read(doc).replace('\n', " ");
    let start = text
        .find("forwards `")
        .expect("a sentence that says what it forwards");
    let sentence = &text[start..];
    let end = sentence.find(". ").unwrap_or(sentence.len());
    routes(
        sentence[..end]
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|path| path.starts_with('/')),
    )
}

#[test]
fn the_guides_name_every_path_the_shipped_origin_forwards() {
    let caddy = caddy_forwards();
    assert!(caddy.contains("/api"), "{caddy:?}");
    for doc in ["docs/user-guide.md", "docs/operator-guide.md"] {
        assert_eq!(guide_forwards(doc), caddy, "{doc}");
    }
}
