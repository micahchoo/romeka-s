# Brief: deploy (production deployment)

## Goal

A person with a server and a domain should be able to run Romeka S in production from the documentation, without
reading the source. That means a container image worth deploying, a compose file that works, a reverse proxy that
terminates TLS, and a runbook for the things that go wrong: backup, restore, upgrade and rollback.

## Scope

Worktree `romeka-s-wt/deploy`, branch `s2/deploy`. Write only `packaging/**` and `deploy/**` (new).

## Context

- `packaging/Dockerfile` exists from the port. Read it first and keep what is right. Check it for: a multi-stage
  build that does not ship a toolchain, a non-root user, an explicit `EXPOSE`, a `HEALTHCHECK` that hits `/health`,
  the SPA built into the image, and a volume for the database and the file store.
- The process is one binary: `romeka serve -d <database> --host --port --base-url`. The SPA and the stored media
  are served by the same process, so one container is the deployment. Read `crates/omeka-server/src/config.rs`
  for every environment variable, including the `ROMEKA_SMTP_*` block, and document each one.
- The account flows need a base URL that matches the origin a browser uses, because the session cookie is
  `SameSite=Lax` and a cookie write must be same-origin. A wrong base URL breaks sign-in in a way that looks like
  a bug. Say so loudly in the docs.
- There is no SMTP dependency. With no SMTP configured the invitation and reset links are written to the log and
  returned to the administrator, so a deployment without mail still onboards users. Document both states.
- The install writes the schema and the default vocabularies: `romeka install`. Upgrading means running the
  migration path, so the runbook covers it.

## Deliverables

- `packaging/Dockerfile` and `packaging/Dockerfile.dockerignore` hardened for production, with the reasons in
  comments where a choice is not obvious.
- `deploy/compose.yaml`: the service, the volumes for the database and the files, the health check, the env block
  with a `.env` file, a restart policy, and no mail service because none is needed.
- `deploy/.env.example`: every variable with a safe default and a one-line comment.
- `deploy/Caddyfile` (or nginx if you prefer, say why): TLS termination, the proxy to the port, and the headers a
  proxy must not break, such as the `Origin` a cookie write needs.
- `deploy/DEPLOY.md`: a first deployment from nothing to a signed-in administrator, then TLS, then backup, restore,
  upgrade and rollback, each as a numbered procedure with the exact commands. Include the SMTP block as optional.
- `deploy/backup.sh` and `deploy/restore.sh`, or a documented pair of `romeka backup` and `romeka restore`
  invocations if the CLI already covers it. Read `crates/omeka-cli/src/commands/backup.rs` and `restore.rs` first.
- A smoke test: `deploy/smoke.sh` that brings the compose stack up, waits for health, signs in as the seeded
  administrator, and tears down. It must be runnable in CI.

## Acceptance

- The smoke test passes on this machine, and you report its exact output.
- Every command in `DEPLOY.md` is one you ran, not one you composed. Say which you could not run and why.
- Report the image size, the run time of the smoke test, and anything the deployment forces on the operator that
  the source does not reveal.

## Verify

- `cd deploy && ./smoke.sh`

## Forbidden

`crates/**`, `web/**`, `docs/**`, the root `README.md`, `LICENSE`, `NOTICE`, `AGENTS.md`, `.github/**`.

## Timebox

4 hours. The image first, then compose and the proxy, then the runbook, then the smoke test.

## Report

Per the repository's AGENTS.md. Include the image size and the smoke test output.
