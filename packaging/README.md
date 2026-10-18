# packaging

The container image: the Romeka S server, and the Caddy origin in front of it
that terminates TLS and serves the admin SPA and the media bytes.

## Files

- `Dockerfile` builds three stages. `node` builds the SPA, `rust` builds the
  release binary of the `omeka-cli` package, and the runtime stage adds Caddy and
  the entrypoint. Build it from the repository root:
  `docker build -f packaging/Dockerfile -t romeka-s:local .`
- `Dockerfile.dockerignore` is the build context filter.
- `entrypoint.sh` installs an absent database, starts the server on loopback and
  Caddy on 80 and 443, and stops the container when either process exits.

## Running it

`deploy/compose.yaml` runs this image with the database, the media store and the
certificates on volumes, and reads its settings from `deploy/.env`. That is the
supported way to run it, and `deploy/DEPLOY.md` is the runbook: first deployment,
TLS, mail, backup, restore, upgrade and rollback.

The image exposes 80 and 443 and nothing else. The server listens on `127.0.0.1`
inside the container, so the origin is the only way in.
