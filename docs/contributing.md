# Contributing

Audience: a developer or an agent about to change this repository.

[`../CONTRIBUTING.md`](../CONTRIBUTING.md) is the human guide and [`../AGENTS.md`](../AGENTS.md) is the
rulebook an agent follows. This page says how the documentation set works, and what a change to it
has to carry. When this page and those two disagree, they win.

## The checks

Run these before you send a change. CI runs the same commands
([`../.github/workflows/ci.yml`](../.github/workflows/ci.yml)):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd web/admin
npm run typecheck && npm run lint && npm test && npm run build && npm run e2e
npm run perf:check

./docs/gen.sh --check
```

Two of them are worth knowing the reason for.

- `npm run e2e` builds the SPA, installs a temporary database, starts the server and drives a real
  browser through the admin and the public site. It is the only check that covers the SPA and the
  server together, so a change to the wire, the router or a page needs it. It also writes
  `web/admin/perf/summary.json`, which `npm run perf:check` reads for the timings.
- `./docs/gen.sh --check` fails when `docs/api.md` no longer matches `crates/omeka-api/src`. Treat
  that as a build failure: the reference is generated from the source and never edited by hand.

## Documentation rules

- `api.md` is generated. Change the source, run `cd docs && ./gen.sh`, and commit both. The
  generator's contract is in [`gen/README.md`](gen/README.md).
- A page names its audience in one line at the top.
- A number lives in the artifact that measures it, not in a page that quotes it. Link the artifact.
  `web/admin/PERF.md` is the exception, because it is the decision log for its own numbers.
- A page that describes code names the source file it was read from.
- A deliberate difference from upstream gets a row in [`compat.md`](compat.md), with the file that
  carries it. That page is the list of differences the product states about itself, and a difference
  that is not there is one a reviewer has to find.

## The generator, and the one check that cannot run here

[`gen/`](gen/) holds the generator behind `api.md`, in Python with no packages. `./gen.sh` writes the
file and `./gen.sh --check` compares it. The generator reads only the crate, so it runs anywhere.

[`gen/check_wire.py`](gen/check_wire.py) cross-checks the parsed members against the captured upstream
documents under `verification/fixtures/api/`. That directory is not in this repository, so the
cross-check cannot run here and the two `omeka-api` test targets that read the same documents fail.
[`gen/check_routes.py`](gen/check_routes.py) needs a running server and an installed database; its
docstring has the commands.

## What a change carries

1. The behaviour change, in the crate or the page that owns it.
2. A test that asserts what a caller observes, with real values. Where it freezes upstream behaviour,
   name it after the upstream file or the behaviour.
3. The generated files, regenerated, when the change touches what they are generated from.
4. A row in [`compat.md`](compat.md) when the change is a deliberate difference from upstream.
5. One commit per coherent step, message `s2(<area>): what changed` in the imperative. No AI
   attribution and no co-author trailers.
6. The commands you ran, and their output, in the pull request or the report. A claim needs a proof
   the reviewer can rerun; a report is a claim, not proof.
