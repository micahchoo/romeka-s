//! Records the git commit the binary was built from, for the backup manifest.
//!
//! The backup manifest names the commit that wrote the archive, so an operator
//! can tell two archives apart. A build outside a checkout reports `unknown`.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest = env!("CARGO_MANIFEST_DIR");
    // Re-run when HEAD moves, so a rebuild after a commit names the new commit.
    // A commit on a branch updates the branch ref rather than HEAD, so watch
    // both paths. `--git-path` resolves the worktree's real files.
    if let Some(head) = git(manifest, &["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(reference) = git(manifest, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(manifest, &["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }
    let commit = git(manifest, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=ROMEKA_COMMIT={commit}");
}

fn git(manifest: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}
