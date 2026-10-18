# Rules for agents working in spike-2

## Poteto mode is on

Read `/home/micah/.agents/skills/poteto-mode/SKILL.md` in full before starting, then the leaf skills your prompt
names for your unit. Apply them; where you make a design call, say which leaf shaped it and what changed.
Your report is a prose surface: short declarative sentences, no em dashes, no emojis, no filler, and every claim
carries its evidence or its label (measured, inferred, or guess).

## Your model reads text only

Never open, read or attach an image, audio or video file. Your model refuses them and the whole session fails.
To check what a page looks like, measure it in a browser test instead: `getBoundingClientRect` for layout,
`getComputedStyle` for colours and fonts. Do not read screenshots back.

## Never read a huge or generated file whole

One request over the model's input limit kills the turn. Check the size first (`wc -c`, `wc -l`), then read a slice
(`sed -n 'A,Bp'`, `head`) or search it (`rg -n`). Never read whole: `source/omeka-s/composer.lock`,
`application/data/install/schema.sql` (read slices), any `node_modules` file, lockfiles, `target/` files,
logs, or any file over ~2000 lines.

## Source of truth

- `DESIGN.md` freezes the architecture, data shapes, crate boundaries, and exclusions. Do not change it.
  If it is wrong, say so in your report; the coordinator edits it.
- `source/omeka-s/` is the fidelity reference. It lives in the main checkout at
  `/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/spike-2/source/omeka-s`; your worktree does not contain it.
  Read it there, never write it, never build it.
- `briefs/<unit>.md` is your brief. `PLAN.md` says where your unit sits.

## Ownership

- Write only the paths your brief names. Other paths belong to other owners.
- Shared files (workspace `Cargo.toml`, `Cargo.lock`, `DESIGN.md`, `PLAN.md`, `VERIFY.md`, `AGENTS.md`, `verify.sh`) are coordinator-owned.
  If your unit needs a change there, put the exact proposed diff in your report; do not edit.
- One owner per crate. Never move another owner's files.

## Commands

- Set a private cargo target dir before any build: `export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/<unit>`.
  It lives on the big disk, not `/tmp`. `/tmp` carries a quota that a handful of cargo target dirs fill, and a
  full `/tmp` fails builds with `Disk quota exceeded` in the middle of a compile.
  Six agents share this workspace; a shared `target/` serialises builds.
- Keep commands scoped: `cargo test -p omeka-core`, `cargo clippy -p omeka-core --all-targets -- -D warnings`,
  `cargo fmt -p omeka-core`. Never run workspace-wide cargo commands; never run `cargo clean`.
- Everything non-interactive: no commands that wait on stdin, no editors, no `docker exec -it`.
  Wrap slow commands in `timeout` (compile loops, docker pulls). `npm ci` not `npm install` when a lockfile exists.
- Never `rm -rf` outside your own temp dirs. Never touch `oracle/` containers unless you own `oracle`.
- Do not start long-lived servers on ports another agent may use. Tests use ephemeral ports (`:0`) or tempdirs.

## Code rules

- Rust: edition 2024, no `unwrap()`/`expect()` outside tests (clippy enforces `unwrap_used`), errors via `thiserror`.
  Repository SQL uses runtime `sqlx::query` (no compile-time macros, no DATABASE_URL at build time).
- TypeScript is strictest (see `web/admin/tsconfig.json`): no `any`, no non-null `!`, no `as` on external data.
- Validation lives at boundaries (HTTP parse, DB row parse, block JSON parse). Inside, types are trusted.
- Tests are behavioural: assert what a caller observes, with real values. Name tests after the upstream file
  or behaviour they freeze where one exists (e.g. `entity_value_lang_tag_matches_upstream`).
- Keep functions under ~80 lines, files under ~600 lines. Split rather than grow.

## Commits

- One commit per coherent step, message `s2(<crate-or-area>): what changed` in the imperative.
  No AI attribution, no co-author trailers, no "generated with" lines, ever.
- Commit only paths you own. Never commit `source/`, `target/`, `node_modules/`, `.db` files.

## Report

End every unit with a report to the coordinator: status (PASS / ISSUES / BLOCKED), files written,
exact commands run with the tail of their output, what you verified, deviations from the brief,
and follow-ups. The coordinator re-runs your tests and reads your diff; a report is a claim, not proof.
