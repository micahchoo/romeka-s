# Changelog

Notable changes to Romeka S. This history starts at the import commit: everything before it is the
Omeka S port this fork was taken from, and it is not replayed here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
semantic versioning.

## [Unreleased]

Nothing yet.

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
