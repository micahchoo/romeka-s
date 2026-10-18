# Deploying Romeka S

One container runs everything. Inside it, `romeka serve` answers on
`127.0.0.1:8123` and Caddy terminates TLS on `:80` and `:443`, serves the admin
SPA out of the image, serves the media files out of the store volume, and
forwards `/api`, `/api-context`, `/assets`, `/s`, `/health` and `/ready` to the
server. Only 80 and 443 leave the container.

Three volumes hold the state:

| Volume | Inside the container | What it holds |
| --- | --- | --- |
| `romeka-db` | `/db` | `romeka.db` with its `-wal` and `-shm` sidecars |
| `romeka-files` | `/data` | the store root `files/`, holding `original/` and the thumbnail directories |
| `romeka-caddy` | `/srv/caddy` | the ACME account and the certificate |

`docker compose down` keeps all three. `docker compose down --volumes` destroys
them, so take a backup first.

Every setting lives in `deploy/.env`, and `deploy/.env.example` documents each
one. The numbered procedures below are for a deployment that already exists,
except the first one.

## Every setting, and who reads it

| Variable | Read by | Notes |
| --- | --- | --- |
| `ROMEEKA_DOMAIN` | Caddy | the name the certificate is issued for |
| `ROMEEKA_BASE_URL` | the server | the public origin; the entrypoint passes it as `--base-url`, and the command line wins over the environment |
| `ROMEEKA_ACME_EMAIL` | Caddy | the ACME account contact; an empty value stops the origin from parsing its config |
| `ROMEEKA_HTTP_PORT`, `ROMEEKA_HTTPS_PORT` | compose | the host ports to publish; the container listens on 80 and 443 |
| `ROMEEKA_IMAGE` | compose | the tag to run, which is what a rollback changes |
| `ROMEEKA_ADMIN_EMAIL`, `ROMEEKA_ADMIN_NAME`, `ROMEEKA_ADMIN_PASSWORD` | the entrypoint | the first administrator, read only by an install |
| `ROMEEKA_TITLE`, `ROMEEKA_TIME_ZONE`, `ROMEEKA_LOCALE` | the entrypoint | written by an install; the install rewrites them every time it runs |
| `ROMEEKA_LOG` | the server | `off`, `error`, `warn`, `info`, `debug` or `trace` |
| `ROMEEKA_MAX_BODY_BYTES` | the server | the request body cap; Caddy sets no smaller one |
| `ROMEEKA_REQUEST_TIMEOUT_SECS` | the server | seconds before the server answers 408 |
| `ROMEEKA_SMTP_HOST`, `ROMEEKA_SMTP_PORT`, `ROMEEKA_SMTP_USER`, `ROMEEKA_SMTP_PASSWORD`, `ROMEEKA_SMTP_TLS`, `ROMEEKA_MAIL_FROM` | the server | mail; with no host the messages go to the log |
| `ROMEEKA_DB`, `ROMEEKA_STORE` | the entrypoint and the server | the database file and the store root, which the volumes in `compose.yaml` are mounted at |
| `ROMEEKA_API_HOST`, `ROMEEKA_API_PORT` | the server and Caddy | the loopback address the server binds and the origin forwards to. The image sets `127.0.0.1:8123`; they are not in the compose environment block, so setting them in `.env` changes nothing. |
| `ROMEEKA_CONFIG` | the server | names a JSON config file. This deployment does not set it: the variables above cover every key it can hold. |
| `ROMEEKA_MODE` | the entrypoint | `serve` (the default) or `install`; set per command, not in `.env` |

An empty value in `.env` means "not configured". The entrypoint drops the empty
ones before it starts the server, because the server reads any value that is
present: an empty `ROMEEKA_SMTP_HOST` would build a relay at the empty address.

## What you need

- A server that runs Docker with the compose plugin (`docker --version` and
  `docker compose version`).
- A domain whose A or AAAA record points at that server.
- Ports 80 and 443 reachable from the internet. Caddy answers the ACME challenge
  on 80 and serves the site on 443.
- An email address for certificate-expiry notices.

## 1. First deployment, to a signed-in administrator

1. Copy the settings and edit them:

   ```
   cd deploy
   cp .env.example .env
   ```

   Set `ROMEEKA_DOMAIN` to your domain, `ROMEEKA_BASE_URL` to the same origin
   with its scheme, `ROMEEKA_ACME_EMAIL` to your address, and the three
   `ROMEEKA_ADMIN_*` values to the account you want to sign in with.

2. Start it:

   ```
   docker compose up --detach --build
   ```

   The first start of an empty deployment writes the schema, the four default
   vocabularies, the Base Resource template and the first administrator. It logs
   `romeka: /db/romeka.db is empty, installing` while it does.

3. Wait for the health check to pass:

   ```
   docker compose ps
   ```

   `STATUS` reads `Up ... (healthy)` within a minute of the container starting.

4. Read the log if it does not:

   ```
   docker compose logs --follow romeka
   ```

5. Check the origin is answering:

   ```
   curl -fsS https://collections.example.org/health
   ```

   The answer is `{"status":"ok"}`.

6. Open `https://collections.example.org/` and sign in with the administrator
   address and password from `.env`.

`ROMEEKA_BASE_URL` must be the origin the browser uses, scheme and port
included, and it must match `ROMEEKA_DOMAIN`. The session cookie is
`SameSite=Lax`, so the server accepts a cookie-carrying write only from that
origin, and the server refuses to answer a cookie write from any other one with
403. Every JSON-LD `@id` is built from it too. A wrong base URL looks like a
server bug: the page loads and sign-in fails. `deploy/smoke.sh` fails on exactly
that check.

## 2. TLS

There is no certificate step. Caddy requests one for `ROMEEKA_DOMAIN` from Let's
Encrypt the first time it starts, answers the challenge on port 80, renews it
before it expires, and stores the account and the certificate in the
`romeka-caddy` volume. Two consequences:

- Keep the `romeka-caddy` volume. Losing it means every certificate is reissued,
  and a certificate authority rate-limits that.
- Port 80 must stay reachable. It carries the challenge and the redirect to
  HTTPS.

To see what Caddy holds:

```
docker compose exec romeka ls /srv/caddy/data/caddy/certificates
```

If no certificate arrives:

1. Check the name resolves here: `dig +short collections.example.org`.
2. Check nothing else holds 80 or 443 on the host: `docker compose ps` and
   `sudo ss -ltnp | grep -E ':(80|443)'`.
3. Read the reason in the log: `docker compose logs romeka | grep -i -E 'certificate|acme|error'`.

For a local deployment with no public name, set `ROMEEKA_DOMAIN=localhost` and
`ROMEEKA_BASE_URL=https://localhost:<published port>`. Caddy issues an internal
certificate that no public authority signed, so a browser warns; `deploy/smoke.sh`
uses `curl --insecure` and needs no warning-free certificate.

## 3. Mail

Mail is optional. With no `ROMEEKA_SMTP_HOST` the server writes the invitation
and the password-reset link to its log and returns the link to the administrator
who asked for it, so a deployment with no relay still onboards users.

To send mail, set the block in `.env` and recreate the container:

```
ROMEEKA_SMTP_HOST=smtp.example.org
ROMEEKA_SMTP_PORT=587
ROMEEKA_SMTP_USER=mailer
ROMEEKA_SMTP_PASSWORD=
ROMEEKA_SMTP_TLS=starttls
ROMEEKA_MAIL_FROM=no-reply@collections.example.org
```

```
docker compose up --detach
```

Then use the "forgot password" link on the sign-in page and watch for the
message:

```
docker compose logs --follow romeka | grep -i mail
```

## 4. Backup

1. Take a backup:

   ```
   ./backup.sh
   ```

   It writes `backup-<UTC stamp>.tar` in this directory and prints what it holds.
   The service keeps serving while it runs: `romeka backup` reads the database
   through one SQLite read transaction (`VACUUM INTO`), so the archive is one
   point in time even with writes in flight, and it checks that every media row
   in that snapshot names a file it has.

2. Name the archive when you want a fixed name:

   ```
   ./backup.sh /srv/backups/romeka.tar
   ```

3. Look inside one without restoring it:

   ```
   tar --list --file backup-20261017T120000Z.tar
   ```

4. Copy it off the machine. The archive is the whole state, so it is the only
   thing a disaster recovery needs besides `.env` and this directory.

A nightly backup needs one line in the operator's crontab:

```
0 3 * * * cd /srv/romeka-s/deploy && ./backup.sh /srv/backups/romeka-$(date +\%F).tar >> /var/log/romeka-backup.log 2>&1
```

`backup.sh` refuses to overwrite an archive, so a repeated name fails loudly
instead of replacing a good backup.

## 5. Restore

Restore replaces the database and mirrors the store, so it is a rollback to the
moment the archive was taken. The service is stopped for the duration, because
restore replaces the database file and a running server holds it open.

1. Take a fresh backup first, so the state you are about to replace is not lost:

   ```
   ./backup.sh before-restore.tar
   ```

2. Restore:

   ```
   ./restore.sh backup-20261017T120000Z.tar
   ```

   The script stops the service, verifies every checksum in the archive against
   the bytes it holds, writes the database and the store, and starts the service
   again. The store is mirrored rather than merged, so a file that was uploaded
   after the archive was taken is removed.

3. Check it is healthy and the accounts are back:

   ```
   docker compose ps
   docker compose exec romeka romeka user list -d /db/romeka.db
   ```

4. Sign in and check the content that changed last is back.

To move a deployment to another machine, install the repository and `.env` on
the new server, run `docker compose up --detach --build` with the empty volumes,
then run `./restore.sh` with the archive. Do not skip the restore: the new
machine starts empty.

## 6. Upgrade

An upgrade is a new image plus the schema migrations, in that order, with the
service stopped for the migration.

1. Back up, and keep that archive under a name you will recognise:

   ```
   ./backup.sh before-upgrade-4.4.0.tar
   ```

2. Build or fetch the new image under a new tag, and set `ROMEEKA_IMAGE` in
   `.env` to it. Building from source on the server keeps the previous image
   tagged and available for a rollback:

   ```
   docker build --file ../packaging/Dockerfile --tag romeka-s:4.4.0 ..
   ```

   Then `ROMEEKA_IMAGE=romeka-s:4.4.0` in `.env`.

3. Stop the service and run the migration:

   ```
   docker compose stop romeka
   docker compose run --rm --no-deps -e ROMEEKA_MODE=install romeka
   docker compose up --detach
   ```

   `ROMEEKA_MODE=install` runs `romeka install` against the existing database,
   which applies the schema migrations, and stops without starting a server.
   The one-off container runs the new image, so the migrations are the new
   release's.

4. Check it: `docker compose ps`, then `curl -fsS https://collections.example.org/health`,
   then sign in.

`install` rewrites the settings rows it owns from `.env`: the installation
title, the administrator email, the time zone and the locale. Keep those values
accurate in `.env`, and expect a title you changed in the admin interface alone
to come back as `ROMEEKA_TITLE`. The rows a server manages as content, the
users, the items and the media, are untouched, and an existing administrator's
password is not changed by an install.

## 7. Rollback

Rollback is the previous image plus the database from before the upgrade.

1. Point `.env` back at the previous image tag: `ROMEEKA_IMAGE=romeka-s:4.3.0`.
2. Restore the archive from step 1 of the upgrade:

   ```
   ./restore.sh before-upgrade-4.4.0.tar
   ```

3. Check it: `docker compose ps`, then sign in.

If the upgrade changed no schema, step 2 is unnecessary and setting
`ROMEEKA_IMAGE` back is the whole rollback. When in doubt, restore: an older
server reading a newer schema is the failure that loses data.

## 8. What this deployment expects of you

- The base URL is the site's origin. It drives the session cookie and every
  `@id`, and a mismatch breaks sign-in. See the end of procedure 1.
- The store is a volume and the origin serves it, so media bytes are only
  present if the store holds them. A restore mirrors the store, and an upgrade
  never touches it.
- The proxy passes the `Origin`, `Referer` and `Cookie` headers through
  unchanged. If you put another proxy in front of this one, it must do the
  same, or every signed-in write is refused with 403.
- Two processes run in the container, the server and Caddy, and the entrypoint
  exits when either dies, so `restart: unless-stopped` restarts a half-dead
  service rather than leaving it answering 502.
- Nothing outside the three volumes and `/tmp` is written, and the container
  filesystem is read-only. An override that writes elsewhere needs a mount.
- Sessions live in the database, not in the container, so a restart does not
  sign anyone out.

## 9. Troubleshooting

| Symptom | First check |
| --- | --- |
| `STATUS` is `unhealthy` | `docker compose logs --follow romeka`, then the `-wal` file in the database volume |
| The browser warns about the certificate | procedure 2, and whether `ROMEEKA_DOMAIN` is the name in the address bar |
| Sign-in fails on a page that loads | `ROMEEKA_BASE_URL` against the address bar, character for character |
| 403 on a save | `ROMEEKA_BASE_URL`, then any proxy in front of this one rewriting `Origin` |
| 502 from the origin | the server is down: `docker compose logs romeka` |
| No mail | procedure 3, and whether the log transport is in use |
| The container restarts in a loop | the first lines of `docker compose logs romeka`: a missing setting exits 1 before the server starts |

A shell in the container, for anything the CLI covers:

```
docker compose exec romeka bash
romeka key add -d /db/romeka.db --owner admin@example.org
```

## Files in this directory

| File | What it is |
| --- | --- |
| `compose.yaml` | the service, the volumes, the ports and the health check |
| `Caddyfile` | TLS, the SPA, the media files and the routes Caddy forwards |
| `.env.example` | every setting, with its default and a one-line comment |
| `backup.sh` | procedure 4, as one command |
| `restore.sh` | procedure 5, as one command |
| `smoke.sh` | builds the image and proves the whole chain on a local port |
| `DEPLOY.md` | this file |
