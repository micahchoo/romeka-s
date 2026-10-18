# Rules for agents working in Romeka S

## What this repository is

Romeka S is a fork of Omeka S: a Rust core and a strict-TypeScript admin, keeping upstream's data
model and API compatible. `README.md` is the front door, `NOTICE` records the provenance, and
`docs/compat.md` is the list of the differences this fork states about itself.

Two kinds of change exist here, and they carry different rules. A change to the wire or the schema
is a compatibility change: `o:` members, response members, status codes and the tables are what an
Omeka S client and database depend on. A change to the implementation behind them is ordinary work.

## Writing your report

Your report is a prose surface. Short declarative sentences, no em dashes, no emojis, no filler.
Every claim carries its evidence or its label in the same sentence: measured, inferred, or guess.
The coordinator re-runs your commands and reads your diff. A report is a claim, not proof.

## Never read a huge or generated file whole

One request over the model's input limit kills the turn. Check the size first (`wc -c`, `wc -l`), then
read a slice (`sed -n 'A,Bp'`, `head`), or search it (`rg -n`). Never read whole: `Cargo.lock`,
`crates/omeka-db/migrations/*.sql`, `web/admin/package-lock.json`, any `node_modules` file, anything
under `target/` or `dist/`, a log, or any file over about 2000 lines.

## Ownership

- Write only the paths your brief names. Every other path belongs to another owner.
- One owner per crate. Never move or edit another owner's files.
- Shared files are coordinator-owned: the workspace `Cargo.toml`, `Cargo.lock`, `README.md`,
  `LICENSE`, `NOTICE`, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `.github/**`
  and `docs/**`. If your unit needs a change there, put the exact proposed diff in your report and
  do not edit the file.
- `docs/api.md` is generated. Change the source under `crates/omeka-api/src/`, run `cd docs &&
  ./gen.sh`, and commit both. Never edit it by hand.

## The crates

| Crate | What it owns |
| --- | --- |
| `omeka-core` | the data model, permissions, data types, value rules. No I/O. |
| `omeka-db` | the schema, the row structs, the repositories. The only crate with SQL, and the only one that names a storage engine. |
| `omeka-api` | the API: routes, adapters, wire types, reads, writes, authentication. |
| `omeka-server` | the HTTP process: the router, the account routes, the public site, mail. |
| `omeka-site` | the public site: views, page blocks, the bundled theme. |
| `omeka-cli` | the `romeka` command line and the job runner. |
| `omeka-media` | file storage and thumbnails. |
| `omeka-migrate` | the adoption path from an Omeka S MySQL database. |
| `omeka-plugin` | the extension seam for adapters and jobs. |

`web/admin` is the SPA, `packaging/` is the container image, `deploy/` is the production runbook,
`docs/` is the documentation and the generator behind `docs/api.md`.

## Source of truth

- The source in `crates/` and `web/` is the truth. A crate README says what that crate owns and where
  it deviates from upstream. Read the README you are changing first.
- `docs/compat.md` records every deliberate difference from upstream, each with the file that carries
  it. Add a row when you add one.
- Upstream Omeka S at `5e413f009c447f4b587803ab23a9cec7d228f6e8` is the fidelity reference for the
  data model and the API. Where upstream source and this repository disagree, upstream wins for
  behaviour, and the disagreement is reported rather than papered over. Read the upstream tree from a
  clone, never write it, never build it.
- A number belongs in the artifact that measures it. Link the artifact rather than quoting it, and
  if you report a number you measured, say what limits it.

## Commands

- Point cargo at a target directory outside the checkout, one per worktree, before you build:
  `export CARGO_TARGET_DIR=<somewhere on the big disk>/<unit>`. A shared `target/` serialises every
  agent that builds, and a small `/tmp` fails a compile with `Disk quota exceeded`.
- Keep cargo scoped: `cargo test -p omeka-api`, `cargo clippy -p omeka-api --all-targets -- -D
  warnings`, `cargo fmt -p omeka-api`. A brief that asks for `cargo test --workspace` is the
  exception, and it says so.
- Everything non-interactive: no command that waits on stdin, no editor, no `docker exec -it`. Wrap a
  slow command in `timeout`. Use `npm ci`, not `npm install`, when a lockfile exists.
- Never `cargo clean`. Never `rm -rf` outside your own temporary directory.
- Tests bind ephemeral ports (`:0`) or use tempdirs. Do not start a long-lived server on a port
  another agent may use, and do not kill processes you did not start.

## Code rules

- Rust: edition 2024, errors through `thiserror`, no `unwrap()` or `expect()` outside tests (clippy
  denies both), and repository SQL through runtime `sqlx::query` so no database is needed to compile.
- TypeScript follows `web/admin/tsconfig.json`: no `any`, no non-null `!`, no `as` on data from
  outside the process. Parse it at the boundary in `web/admin/src/api/parse.ts`.
- Validation lives at boundaries: the HTTP parse, the DB row parse, the block JSON parse. Inside,
  types are trusted and nothing re-validates.
- Tests assert what a caller observes, with real values. Where a test freezes upstream behaviour,
  name it after the upstream file or the behaviour.
- Keep functions under about 80 lines and files under about 600. Split rather than grow.

## Commits

- One commit per coherent step, message `s2(<crate-or-area>): what changed` in the imperative.
- No AI attribution, no co-author trailers, no "generated with" lines, ever.
- Commit only the paths your brief names. Never commit `target/`, `node_modules/`, `dist/`, a `.db`
  or `.sqlite` file, a log, or a captured credential.
- `Cargo.lock` is committed, because the image builds with `cargo build --locked`. Update it with
  cargo as part of a dependency change; never by hand.

## Report

End every unit with a report to the coordinator:

- Status: PASS, ISSUES or BLOCKED.
- The files written.
- The exact commands run, each with the tail of its output.
- What you verified, and against which artifact.
- Every deviation from your brief.
- Follow-ups, including anything you found that belongs to another owner.
