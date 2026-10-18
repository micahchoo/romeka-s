# Changelog

Notable changes to Romeka S. This history starts at the import commit: everything before it is the
Omeka S port this fork was taken from, and it is not replayed here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
semantic versioning.

## [Unreleased]

### Changed

- Corrected the pages that still called the golden fixtures missing and two `omeka-api` test targets
  red: the fixtures are at [`fixtures/api/`](fixtures/api/README.md), `docs/gen/check_wire.py` reads
  them and exits non-zero on an empty set, and the README's "What is not ported" now names the admin
  SPA gaps.
- Added two rows to [`docs/compat.md`](docs/compat.md) for the deliberate differences: a write that
  names an unknown property term answers 422, and `GET /api` answers JSON-LD rather than HTML.

### Fixed

- A `PATCH` that names no value object keeps every stored value; one that names a valid value still
  replaces the whole set, as upstream's `ValueHydrator` does.
- Every reference a body names resolves before the first write, so a refused reference leaves the
  stored row untouched and answers the 404 `findEntity` throws.
- A site `PUT` removes every page its `o:page` list omits, and so does a `PATCH` that carries the
  member.
- A `DELETE` answers the detached entity, with `o:id` null and `@id` the collection URL, the shape
  `Api\Manager::finalize` renders after the flush.
- The admin's delete controls consume that detached answer, so the twelve `delete*` methods return
  `void` and the boundary only checks the shape (`assertDetached` in `web/admin/src/api/parse.ts`).
- An email that is not `local-part@hostname` answers the Laminas `INVALID_FORMAT` message.
- The `id` filter narrows every entity collection, and `resource_classes` and `properties` read
  `term`, `vocabulary_prefix` and `vocabulary_namespace_uri`.
- A non-numeric `page` leaves a list unbounded, because `SetBrowseDefaults` writes `page=1` only
  when the caller sent none.
- `api_resources`, `data_types` and `modules` answer no `Omeka-S-Total-Results` header and ignore
  the page window, because their adapter answers a plain `Response`.
- The `Link` header renders nested and repeated parameters the way Laminas does, through the
  request array `read/php.rs` rebuilds.
- Text sorts order through one case-fold rule (`repos::compare_text_ci` in `omeka-db`) that matches
  SQLite's `COLLATE NOCASE`, so a Rust comparator and a `COLLATE NOCASE` clause agree.
- A value object's `@annotation` reads and writes a `value_annotation` resource, dropped when the
  value that carried it leaves the set.

## [0.1.0] - 2026-10-17

The import. Romeka S is the Omeka S port's tree under its own name, with the same data model, the
same `/api` surface and the same license as upstream Omeka S, forked at
`5e413f009c447f4b587803ab23a9cec7d228f6e8` (see [`NOTICE`](NOTICE)).

### Added

- The account flows. Sign in and sign out on the admin header, with the session held in a
  server-side row behind an `HttpOnly`, `SameSite=Lax` cookie; `GET /api/me` as the session probe;
  the invite and forgot-password links; and the set-password page behind its one-time key. With no
  SMTP relay configured, the invite and reset links are written to the server log and returned to
  the administrator, so an installation without mail onboards people anyway
  ([`crates/omeka-server/README.md`](crates/omeka-server/README.md), Accounts).
- The admin sign-in panel carries both forms: the API key pair and the password login.
- Set-password and forgot are their own lazy chunks, so a reader of an admin page downloads neither.
