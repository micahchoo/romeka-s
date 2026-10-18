# Brief: forwardport (Romeka S carries the port's fidelity wave)

## Goal

The port repository (`../spike-2`, branch `main`) landed a wave of behaviour fixes after the Romeka S
import: PATCH keeps stored values, references resolve before any write, site PUT drops omitted pages,
DELETE answers the detached entity, the Laminas email message, the `id`, `term` and `vocabulary_*`
filters, a non-numeric `page`, no total header on plain collections, Laminas `Link` encoding, case-folded
text sorts, and value annotations. Carry every one of them into this repository so the two trees differ
only by the product rename, and prove it with this repository's gates and the port's 82-step oracle set.

## Scope

Worktree `romeka-s-wt/forwardport`, branch `s2/forwardport`. Write only `crates/**`, `docs/api.md`
(generated: run `cd docs && ./gen.sh`, never edit by hand), `CHANGELOG.md`, and `Cargo.lock` through
cargo only. Read anything in both repositories.

## Context

- The import commit `a99e16f` here equals spike-2 `6f90cc8` minus the harness. Everything spike-2 added
  under `crates/` since then is the payload: `git -C ../spike-2 log --oneline 6f90cc8..main -- crates`
  lists the commits, `git -C ../spike-2 diff 6f90cc8..main --stat -- crates` the files.
- The rename this tree carries (`briefs/brand.md`): the binary is `romeka`, env prefix `ROMEKA_`, cookie
  `romeka_session`, database file `romeka.db`, product name `Romeka S` in identity strings, user agent
  `romeka-s/`. Wire facts stay upstream's (`o:` prefix, `Omeka-S-Version`, `Omeka\Entity\...` strings).
  `crates/omeka-server/tests/product_identity.rs` guards both sides.
- Suggested method: `git -C ../spike-2 format-patch -o <tmp> 6f90cc8..main -- crates` then `git am -3`
  here. Patches that touch a renamed line will conflict; resolve each by taking the port's logic with this
  tree's identity strings. Record every conflict file in the report. Do not rebase or rewrite history;
  `git am` on your own branch is fine.
- Prove equality, not just a clean apply: normalise this tree's `crates/` with the rename mapping and
  `diff -r` it against spike-2's `crates/`. The only remaining differences must be identity strings, the
  `fixtures/api/` path in two tests, the product_identity test, and the crate `description` lines.
- The port's 82-step scenario set lives at `../spike-2/verification/scenarios/`. Its `diff.sh --base URL`
  runs against any server that speaks the API; issue the dev key with its `issue_api_key.py --database`
  on your served database. The expected verdict against a fixed tree is
  `82 steps: 79 VERIFIED, 0 DIFF, 0 NOT VERIFIED, 3 EXCLUDED` with `--exclude exclusions.txt`
  (`compare.py` takes the flag; `serve_port.sh` shows how). The oracle at `http://localhost:8899` does
  not need to be up for that.
- `docs/api.md` is generated from `crates/omeka-api/src`; after the port lands, regenerate it and commit
  it with the code. CI runs `./docs/gen.sh --check`.

## Acceptance

- The normalised `diff -r` of `crates/` against spike-2 shows only the allowed differences, pasted in the
  report.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` green.
- `cd web/admin && npm run e2e` green (the browser suite drives the changed server).
- `cd docs && ./gen.sh --check` green after regeneration.
- The 82-step set against a served build of this branch: 79 VERIFIED, 3 EXCLUDED.
- `CHANGELOG.md` Unreleased gains one line per behaviour above, under Fixed.

## Verify

```
export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/romeka-forwardport
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cd docs && ./gen.sh --check
cd web/admin && npm ci && npm run e2e
# then serve the binary and run ../spike-2/verification/scenarios/diff.sh --base against it
```

## Timebox

3 hours.

## Forbidden

`web/**` (nothing in the wave touched it), `docs/**` other than `api.md`, `README.md`, `fixtures/**`,
`deploy/**`, `packaging/**`, `.github/**`, `briefs/**`. No rebase of existing commits, no merge, no
force-push, never commit on `main`.

## Report

Per `AGENTS.md`, written to `REPORT.md` in your worktree, uncommitted. Include the conflict list, the
normalised diff, and the 82-step verdict line.

## Standing

Read `/tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/d1ed6352-35eb-4eb2-bda3-5b514e62af4b/scratchpad/orchestrate/omeka-port-fidelity/preferences.md`.
