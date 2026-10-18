# Brief: forwardport2 (the admin accepts upstream's DELETE answer, here too)

## Goal

`forwardport` carried the port's fidelity wave into this tree, and with it upstream's DELETE answer: the
detached entity with `o:id` null. The admin SPA here still parses that body as a live resource, so nine
browser specs fail with the alert `$.o:id: expected number`. The port fixed its SPA in the `spadelete`
unit (spike-2 branch `s2/spadelete`, merged to spike-2 `main`). Carry that fix here so `npm run e2e` is
green again.

## Scope

Worktree `romeka-s-wt/forwardport2`, branch `s2/forwardport2`, created from `main` after `forwardport`
merged. Write only `web/admin/src/api/**`, `web/admin/tests/**`, `web/admin/README.md`, and
`web/admin/e2e/specs/**` only where the port's fix changed them. Read anything.

## Context

- The payload: `git -C ../spike-2 log --oneline <spike-2 main before spadelete>..main -- web/admin`, or
  simply the commits on spike-2 branch `s2/spadelete`
  (`git -C ../spike-2 log --oneline main..s2/spadelete` shows nothing once merged, so use
  `git -C ../spike-2 log --oneline --first-parent -3 main` and read the merge named `spadelete`).
- Method as before: `git fetch ../spike-2 main`, `format-patch` the spadelete commits restricted to
  `web/admin`, `git am -3`. The SPA here is renamed in its document title and i18n catalogue only; the
  API client is byte-identical to the port's, so no conflict is expected. Report any.
- Prove equality: `diff -r` of `web/admin/src/api` and `web/admin/tests` against spike-2 `main` must be
  empty after the rename mapping (`Romeka S` to `Omeka S`, `romeka_session` to `omeka_session`).

## Acceptance

- `cd web/admin && npm run typecheck && npm run lint && npm test && npm run build && npm run perf:check` green.
- `cd web/admin && npm run e2e` green, 45 specs. The harness builds the server from this worktree; set
  `CARGO_TARGET_DIR` first.
- `CHANGELOG.md` Unreleased, Fixed: one line for the admin's delete flow.

## Verify

```
export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/romeka-forwardport2
cd web/admin && npm ci && npm run typecheck && npm run lint && npm test && npm run build && npm run perf:check && npm run e2e
```

## Timebox

1 hour.

## Forbidden

`crates/**`, `web/admin/src/pages/**`, `web/admin/src/app/**`, `docs/**`, `README.md`, `fixtures/**`,
`deploy/**`, `packaging/**`, `.github/**`, `briefs/**`. No rebase of existing commits, no merge, never
commit on `main`.

## Report

Per `AGENTS.md`, written to `REPORT.md` in your worktree, uncommitted. Include the e2e summary line.

## Standing

Read `/tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/d1ed6352-35eb-4eb2-bda3-5b514e62af4b/scratchpad/orchestrate/omeka-port-fidelity/preferences.md`.
