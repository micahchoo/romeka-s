# packaging

The Omeka S port as a service. The image runs two processes. `omeka serve` is
the API and the public site pages on `127.0.0.1:8123`. nginx is the static
origin on port 8080. It serves the admin bundle at `web/admin/dist`, serves the
stored media bytes under `/files/`, and forwards `/api`, `/health` and `/s` to
`omeka serve`. The database and the media store live on volumes, never inside
the image.

## Files

- `Dockerfile` builds three stages. `node` builds the SPA, `rust` builds the
  `omeka` release binary, and the runtime stage adds nginx and the entrypoint.
- `Dockerfile.dockerignore` is the build context filter.
- `compose.yaml` runs the service with a data volume and a store volume.
- `entrypoint.sh` resolves the paths, runs `omeka install`, starts both
  processes, and stops the service when either exits.
- `nginx.conf.template` is the origin config. `entrypoint.sh` renders it.
- `omeka.service` is the systemd unit for a bare-metal install.
- `smoke.sh` builds the image, runs it, checks health, the page and the API,
  restarts it, and tears it down.

## Ports

| Port | Who | Reachable |
| --- | --- | --- |
| 8124 | the origin, mapped from 8080 | the host, published by compose |
| 8080 | nginx inside the container | the container network only |
| 8123 | `omeka serve` inside the container | loopback inside the container |

The API is reached through the origin, so only one port is published. The
container health check fetches `http://127.0.0.1:8080/health`.

## Quick start

From the repository root:

```
docker compose -f packaging/compose.yaml up --build
```

The origin is then at `http://localhost:8124/`. Open that URL for the admin
SPA, `http://localhost:8124/api/items` for the API, and
`http://localhost:8124/health` for liveness.

`up` builds the image on first run. A later `up --build` rebuilds it. `down`
stops the service and keeps both volumes.

## First run

The first run means `omeka install`. The entrypoint runs it on every start
against the path in `OMEKA_DB`. `install` creates the database, runs the
migrations, seeds the four default vocabularies and the `Base Resource`
template, and writes the first `global_admin`. It is idempotent, so a restart
against a seeded database inserts nothing and changes nothing.

The admin account comes from the environment, which compose sets:

| Variable | Default | Used by |
| --- | --- | --- |
| `OMEKA_ADMIN_EMAIL` | `admin@example.com` | `omeka install` |
| `OMEKA_ADMIN_NAME` | `Admin` | `omeka install` |
| `OMEKA_ADMIN_PASSWORD` | `changeme` | `omeka install` |
| `OMEKA_TITLE` | `Omeka S` | `omeka install` |
| `OMEKA_BASE_URL` | `http://localhost:8080` | `omeka serve`, the JSON-LD `@id` |

Change `OMEKA_ADMIN_PASSWORD` in `compose.yaml` before the first `up`. The
value is only read when the database is first seeded, so a later change does
not reset an existing password. Use `omeka user set-password -d /data/omeka.db
--email EMAIL --password PASSWORD` for that.

## Mint an API key

`omeka key add` prints the credential once. Run it in the container:

```
docker compose -f packaging/compose.yaml exec omeka \
    omeka key add -d /data/omeka.db --owner admin@example.com
```

The two printed values are `key_identity` and `key_credential`. Send them to
the API as query parameters:

```
curl "http://localhost:8124/api/items?key_identity=ID&key_credential=SECRET"
```

`omeka key list -d /data/omeka.db` shows the identities without the
credentials. `omeka key revoke -d /data/omeka.db --identity ID` removes one.

## Import content

Copy the files into the container, then run the import against the store
volume:

```
docker compose -f packaging/compose.yaml cp ./collection omeka:/tmp/collection
docker compose -f packaging/compose.yaml exec omeka \
    omeka import media -d /data/omeka.db --source /tmp/collection \
        --store /data/files --item-set 1
```

`--store /data/files` is the store volume, the same path nginx serves under
`/files/`. Items from a CSV use `omeka import csv -d /data/omeka.db --file
FILE --item-set ID`. The `README.md` in `crates/omeka-cli` documents every
command.

## Backup and restore

The state is two things, the SQLite database and the media store. Back up both
from the volumes.

```
docker compose -f packaging/compose.yaml stop
docker run --rm -v omeka-s_omeka-data:/data -v "$PWD":/backup alpine \
    tar -C /data -czf /backup/omeka-data.tgz omeka.db files
```

The database is one file. Stop the service before you copy it, or copy
`omeka.db`, `omeka.db-wal` and `omeka.db-shm` together. The store is the
directory `files` beside the database, which holds `original/` and the
thumbnail directories.

Restore into fresh volumes and start again:

```
docker volume create omeka-s_omeka-data
docker run --rm -v omeka-s_omeka-data:/data -v "$PWD":/backup alpine \
    tar -C /data -xzf /backup/omeka-data.tgz
docker compose -f packaging/compose.yaml up
```

The volume names take the compose project name from `name: omeka-s`, so they
are `omeka-s_omeka-data` and `omeka-s_omeka-store`. `docker volume ls` lists
the real names.

## Bare metal with systemd

Install the pieces by hand. `omeka` is the binary from `cargo build --release
-p omeka-cli` at `target/release/omeka`, and the dist comes from `npm run
build` in `web/admin`:

```
install -m 0755 target/release/omeka    /usr/local/bin/omeka
install -m 0755 packaging/entrypoint.sh /usr/local/bin/omeka-entrypoint
install -d /etc/omeka /usr/share/omeka/dist
install -m 0644 packaging/nginx.conf.template /etc/omeka/nginx.conf.template
cp -r web/admin/dist/. /usr/share/omeka/dist/
install -m 0644 packaging/omeka.service /etc/systemd/system/omeka.service
```

The unit needs the `omeka` system user, nginx, and curl:

```
useradd --system --user-group --home-dir /var/lib/omeka --shell /usr/sbin/nologin omeka
apt-get install -y nginx curl
```

Write the admin account and the public URL into `/etc/omeka/omeka.env`:

```
OMEKA_ADMIN_EMAIL=admin@example.com
OMEKA_ADMIN_NAME=Admin
OMEKA_ADMIN_PASSWORD=changeme
OMEKA_TITLE=Omeka S
OMEKA_BASE_URL=http://localhost:8080
```

Then enable and start the unit:

```
systemctl daemon-reload
systemctl enable --now omeka.service
```

systemd creates `/var/lib/omeka` for the database and the store, and
`/run/omeka` for the rendered nginx config. The unit restarts on failure and
polls `/health` after start, the same check the container runs. The unit runs
nginx as the `omeka` user, so it listens on 8080 rather than 80. Put a TLS
proxy in front for a public host.

## What the build assumes about the host

- Docker with BuildKit, and the ability to pull `node:22-bookworm-slim`,
  `rust:1.93-bookworm` and `nginx:1.27-bookworm`.
- Network access to the npm registry and to crates.io during the build. The
  runtime needs no network access.
- A `linux/amd64` host, because the base images are.
- About 3 GB of free disk. The image is 207 MB, and a cold build writes the
  Cargo compiler cache.
- The repository root as the build context. `smoke.sh` runs the build for you.
- `git` and `curl` on the host for `smoke.sh`.

The runtime image mounts nothing from the repository. It writes only to `/data`
and `/data/files`, which compose maps to volumes. The build reads the context
and does not modify it.

## Limitations

- The entrypoint runs `omeka install` on every start. That is idempotent, but it
  opens and migrates the database on each start. Set `OMEKA_SKIP_INSTALL=1` to
  skip it once the database exists.
- Thumbnails are only built at import time. Run `omeka media thumbnails -d
  /data/omeka.db --store /data/files` for rows imported before the thumbnailer.
- The origin does not inject an API key. The admin SPA sends its own, and the
  public read endpoints need none.
