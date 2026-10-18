# Contributing

Audience: anyone about to change this repository. An agent working here follows
[`AGENTS.md`](AGENTS.md), which carries the same rules in more detail.

## Set up

```
cargo build --workspace          # Rust 1.93 or newer, edition 2024
cd web/admin && npm ci           # Node 22 or newer, installs from package-lock.json
```

A change is ready when these are green, which is what
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on every pull request:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd web/admin
npm run typecheck
npm run lint
npm test
npm run build
npm run e2e
npm run perf:check
```

`npm run e2e` builds the SPA, installs a temporary database, starts the server and drives a real
browser through the admin and the public site. It is the only check that covers the two together, so
run it before you send a change that touches the wire, the router or a page.

Use a separate `CARGO_TARGET_DIR` if you build in more than one checkout at a time. Two builds
sharing one target directory serialise against each other.

## Layout

| Path | What lives there |
| --- | --- |
| `crates/omeka-core` | the data model, permissions, data types and value rules, with no I/O |
| `crates/omeka-db` | the schema, the row structs and the repositories. It is the only crate with SQL, and the only place that names a storage engine |
| `crates/omeka-api` | the API: routes, adapters, wire types, reads, writes, authentication |
| `crates/omeka-server` | the HTTP process: the router, the account routes, the public site, mail |
| `crates/omeka-site` | the public site: views, page blocks, the bundled theme |
| `crates/omeka-cli` | the `romeka` command line and the job runner |
| `crates/omeka-media` | file storage and thumbnails |
| `crates/omeka-migrate` | the adoption path from an Omeka S MySQL database |
| `crates/omeka-plugin` | the extension seam for adapters and jobs |
| `web/admin` | the admin SPA |
| `docs` | the guides, and the generator behind `docs/api.md` |
| `packaging`, `deploy` | the container image and the production runbook |

Each crate has a README that says what it owns and where it deviates from upstream. Read the one you
are changing first.

## Rules

- **Keep the wire compatible.** The `o:` prefix, the response members, the status codes and the
  schema are what an Omeka S client and database depend on. A change there is a compatibility change:
  say so in the pull request, and update [`docs/compat.md`](docs/compat.md) if the difference is
  deliberate.
- **Validation lives at boundaries.** The HTTP parse, the DB row parse and the block JSON parse
  validate and narrow. Inside, the types are trusted and there is no re-checking.
- **Rust.** No `unwrap()` or `expect()` outside tests (clippy denies both). Errors are `thiserror`
  types. Repository SQL is runtime `sqlx::query`, so no database is needed to compile.
- **TypeScript follows `web/admin/tsconfig.json`.** No `any`, no non-null `!`, and no `as` on data
  that arrived from outside. Parse it in `src/api/parse.ts` and let the types carry it from there.
- **Tests assert what a caller observes**, with real values. Where a test freezes upstream behaviour,
  name it after the upstream file or the behaviour, the way the existing tests do.
- **One commit per coherent step**, message `s2(<area>): what changed` in the imperative. Do not put
  generated files in a commit of their own when they belong to the change beside them.

## Documentation

- [`docs/api.md`](docs/api.md) is generated from the source. Never edit it by hand: change the
  source under `crates/omeka-api/src/`, run `cd docs && ./gen.sh`, and commit both. `./gen.sh --check`
  fails when the file is stale, and CI runs it.
- [`docs/compat.md`](docs/compat.md) is where a deliberate difference from upstream is recorded. Add
  a row when you add a difference, with the file that carries it.
- A number belongs in the artifact that measures it, not in a page that quotes it. Link the artifact.
  `web/admin/PERF.md` is the exception: it is the decision log for the numbers it records.

## Reporting a problem

A bug that is not a security problem goes to the issue tracker with the exact command, the version
(`romeka --version`) and what you saw. A security problem goes to the private channel in
[`SECURITY.md`](SECURITY.md), not the issue tracker.
