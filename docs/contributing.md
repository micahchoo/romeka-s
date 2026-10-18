# Contributing

Audience: an agent or a developer about to change this repository.

[`../AGENTS.md`](../AGENTS.md) is the rulebook and [`../PLAN.md`](../PLAN.md) is the sequencing. This page says how the
two fit together and what a change has to carry to be accepted. When this page and those two disagree, they win.

## How work is organized

The port is built by one coordinator and one agent per unit of work.

- [`../PLAN.md`](../PLAN.md) sequences the units into waves, names the owner and the acceptance of each one, and keeps a
  dated log of what landed and what a merge taught the coordinator.
- Each unit has a brief, `../briefs/<unit>.md`, written before the unit's worktree exists. The brief names the goal, the
  scope, the deliverables, the acceptance, the verify command and what is forbidden.
- [`../ROADMAP.md`](../ROADMAP.md) ranks the whole territory that is not in flight yet.

## Your workspace

- One git worktree per unit, `../spike-2-wt/<unit>`, on branch `s2/<unit>`. The unit named in your brief is the only
  place you write.
- One owner per crate, and one owner per verification artifact. Never move or edit another owner's files.
- [`../DESIGN.md`](../DESIGN.md) is frozen and coordinator-owned: the architecture, the data shapes, the crate
  boundaries and the exclusions. If it is wrong, say so in your report; the coordinator edits it.
- [`../source/omeka-s`](../README.md) (the upstream clone) is the fidelity reference. Read it, never write it, never
  build it.
- The workspace manifest, `Cargo.lock`, `DESIGN.md`, `PLAN.md`, `VERIFY.md`, `AGENTS.md` and `verify.sh` are
  coordinator-owned. Put the proposed diff in your report instead of editing them.

## Commands

[`../AGENTS.md`](../AGENTS.md) has the full list. The four that cost the most when ignored:

- Export a private `CARGO_TARGET_DIR` under `/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/<unit>` before
  building. Six agents share the workspace, and a shared target directory serialises them.
- Scope every cargo command to your crate (`cargo test -p omeka-core`, `cargo clippy -p omeka-core --all-targets -- -D
  warnings`). Never run a workspace-wide cargo command, and never `cargo clean`.
- Keep everything non-interactive. Wrap a slow command in `timeout`, use `npm ci` when a lockfile exists, and let tests
  pick ephemeral ports.
- Never commit `Cargo.lock`, `source/`, `target/`, `node_modules/` or a database file.

## What an accepted change carries

1. The deliverable the brief names, in the paths the brief names.
2. The brief's acceptance, run for real. [`../VERIFY.md`](../VERIFY.md) sets the standard: a claim needs a proof the
   checker reruns, a verdict is VERIFIED, NOT VERIFIED or INCONCLUSIVE, and inconclusive is not a pass.
3. Tests that assert what a caller observes, named for the upstream file or behaviour they freeze where one exists.
4. One commit per coherent step, message `s2(<crate-or-area>): what changed` in the imperative. No AI attribution, no
   co-author trailers.
5. A report in the format [`../AGENTS.md`](../AGENTS.md) gives: status, the files written, the exact commands with the
   tail of their output, what you verified, your deviations from the brief, and follow-ups.

The coordinator reads the diff, re-runs the commands, and confirms on the real artifact. A report is a claim, not
proof.

## Merge rules

- The coordinator merges one verified unit at a time onto `main`. Agents never merge, rebase or push.
- A unit that consumes a crate another unit owns is sequenced after that unit, or the shared surface is frozen before
  both start. [`../PLAN.md`](../PLAN.md) records what happened when that rule was broken: two units each passed alone
  and the merged workspace did not compile.
- The gate is `./verify.sh`, and it must be green before a wave closes. `git diff --quiet` after a generator run is
  part of it for any unit that generates a file.

## Documentation-specific rules

- [`api.md`](api.md) is generated. Change the source under `../crates/omeka-api/src/`, run `cd docs && ./gen.sh`, and
  commit both. The generator's contract is in [`gen/README.md`](gen/README.md).
- Documentation links the artifact that carries a number or a list. It does not copy it, because a copy drifts and the
  link does not. The acceptance report, the scoreboard and the matrices are the artifacts.
- A document here names its audience in one line at the top.
