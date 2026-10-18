# Brief: brand (the Romeka S rename)

## Goal

The product is called Romeka S. Rename the identity of this software everywhere it names itself, and leave
untouched everything that names upstream's data model, because that is what makes this a compatible fork.

## Scope

Worktree `romeka-s-wt/brand`, branch `s2/brand`. Write only `crates/**` and `web/**`.

## The line between identity and compatibility

Rename these, because they name **this** software:

- The binary: `crates/omeka-cli/Cargo.toml` declares it, so the command becomes `romeka`.
- The environment prefix: `ROMEKA_` in place of `OMEKA_` for every variable the server reads, including the mail
  variables `OMEKA_SMTP_*` and `OMEKA_MAIL_FROM`.
- The default database file name: `romeka.db`.
- The session cookie name: `romeka_session`.
- The admin SPA's document title, the shell's title, the product name in the i18n catalogue, the theme's
  "Powered by" line, the CLI's about and help text, the package metadata in `Cargo.toml` (description, repository,
  homepage), and the crate READMEs.

Do **not** rename these, because they are compatibility facts a client or a database depends on:

- The JSON-LD `o:` prefix, the `@context` path, and the `Omeka-S-Version` response header.
- Class names that appear on the wire, such as `Omeka\Entity\User` in a permission error document. A client may
  match that string, so it stays byte for byte.
- Anything in the schema, the migration path, or the API member names.
- Any sentence that credits Omeka S as the origin. The license section keeps that credit; do not remove it.

Crate names stay as they are (`omeka-core`, `omeka-api`, and so on). Say so in your report if that reads wrong to
you, with the reason, but do not change them.

## Deliverables

- The rename across `crates/**` and `web/**`, with the tests updated where they assert a renamed string.
- A test that fails if the product name and the compatibility strings drift: assert that a rendered page still
  carries the `o:` prefix and the version header, and that the product's own title carries Romeka S. This is the
  guard that stops a later rename from eating the wire.
- Every crate README updated, with the product named once and upstream credited once.
- Report the file count and the exact list of strings you renamed, and the list you deliberately left.

## Acceptance

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green.
- `cd web/admin && npm ci && npm run typecheck && npm run lint && npm test && npm run build && npm run e2e` green.
- Report anything the rename could not reach.

## Verify

- `export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/romeka-brand`
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
- `cd web/admin && npm ci && npm run typecheck && npm run lint && npm test && npm run build && npm run e2e`

## Forbidden

`docs/**` (another unit owns the documentation), `deploy/**`, `packaging/**`, the root `README.md`, `LICENSE`,
`NOTICE`, `AGENTS.md`, `.github/**`, `Cargo.lock` except through `cargo` itself.

## Timebox

4 hours. The code strings first, then the tests, then the READMEs.

## Report

Per the repository's AGENTS.md. Include the renamed and the preserved lists.
