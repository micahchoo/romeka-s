# Brief: docsromeka (the product pages tell the truth about the tree)

## Goal

Four pages still say the golden fixtures are missing and two test targets are red. They landed at
`fixtures/api/` in commit `fa5d485` and `cargo test --workspace` passes 1150 tests. The README's "What
is not ported" omits the admin surface the port's own matrix marks as missing. Correct both.

## Scope

Worktree `romeka-s-wt/docsromeka`, branch `s2/docsromeka`. Write only `docs/**`, `README.md`,
`CHANGELOG.md`, `web/admin/README.md`, and the one comment line at `web/admin/tests/parsers.test.ts:3`.
Read anything.

## Context

- Stale claims: `docs/compat.md` ("Where the evidence is"), `docs/contributing.md` (the `check_wire.py`
  paragraph), `docs/gen/README.md`, `docs/operator-guide.md` (the `cargo test` paragraph),
  `web/admin/README.md:327`, `web/admin/tests/parsers.test.ts:3`. Each names `verification/fixtures/api/`.
  The directory is `fixtures/api/` at the repository root and `fixtures/api/README.md` explains it.
- `docs/gen/check_wire.py:45` still reads `REPO / "verification" / "fixtures" / "api"`, finds nothing,
  prints `0 fixtures checked` and exits 0. Point it at `fixtures/api` and make it exit non-zero when it
  finds no fixture, so it can never pass vacuously again.
- The port's surface matrix (`../spike-2/verification/upstream-inventory/exclusions.txt`, groups
  `admin-batch`, `admin-import`, `admin-job-stop`, `asset-upload`, `site-admin`, `upstream-search`,
  `auth`) names what the admin SPA does not offer: no search, filter or sort controls on any list;
  no batch edit or batch delete; no module install, activate, deactivate, configure or uninstall; no
  vocabulary or resource template import or export; no job stop; no asset upload; no site users and
  permissions editor; no maintenance mode. Say each in "What is not ported", plainly, and keep the
  existing items.
- Two deliberate differences from upstream found on 2026-10-17 have no row in `docs/compat.md`: an
  unknown property term in a write answers 422 with a typed error where upstream answers 500 HTML
  (`crates/omeka-api/src/write/values.rs`), and `GET /api` answers JSON-LD where upstream answers an
  HTML page (`crates/omeka-api/src/read/`). Add both rows with the file that carries each.
- The behavioural fixes being made in the port repository (`../spike-2`, units writefix, readfix,
  collation) will be forward-ported by another unit later. Do not describe them yet.

## Deliverables

1. The six stale references corrected, and the two `cargo test` claims replaced by what runs: the
   whole workspace, with `fixtures/api/` read by `golden.rs` and `property_wire.rs`.
2. `check_wire.py` reads `fixtures/api/` and fails on an empty set.
3. README "What is not ported" extended as above.
4. `docs/compat.md` gains the two rows.
5. `CHANGELOG.md` Unreleased: one line for the documentation correction, one for the compat rows.

## Acceptance

- `grep -rn "verification/fixtures" docs README.md web/admin/README.md web/admin/tests` finds nothing.
- `cd docs && ./gen.sh --check && python3 gen/check_wire.py --quiet` prints `16 fixtures checked` and
  exits 0; with an empty directory argument it exits non-zero (show the command).
- `cd web/admin && npm test` green.

## Verify

```
cd docs && ./gen.sh --check && python3 gen/check_wire.py --quiet
cd web/admin && npm test
```

## Timebox

1.5 hours.

## Forbidden

`crates/**`, `web/admin/src/**`, `web/admin/e2e/**`, `fixtures/**`, `deploy/**`, `packaging/**`,
`.github/**`, `AGENTS.md`, `LICENSE`, `NOTICE`, `Cargo.lock`. No rebase, no merge, never commit on `main`.

## Report

Per `AGENTS.md`, written to `REPORT.md` in your worktree, uncommitted.

## Standing

Read `/tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/d1ed6352-35eb-4eb2-bda3-5b514e62af4b/scratchpad/orchestrate/omeka-port-fidelity/preferences.md`.
