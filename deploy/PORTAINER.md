# Deploying Romeka S with Portainer

This is the Portainer version of [`DEPLOY.md`](DEPLOY.md). The container, the
volumes and the settings are the same. What changes is that Portainer pulls a
published image instead of building one, and that every step is done in the
Portainer UI where it can be.

## 1. The image

Portainer pulls the image; it does not build it. The `image` workflow
(`.github/workflows/image.yml`) publishes two images to the GitHub Container
Registry on every push to `main` and on every `v*` tag:

| Image | What it is |
| --- | --- |
| `ghcr.io/micahchoo/romeka-s` | the server, with Caddy and the admin |
| `ghcr.io/micahchoo/romeka-s-office-tools` | the optional office worker |

Each push to `main` is tagged `latest` and `sha-<commit>`. A tag `v1.2.3` is
also tagged `1.2.3` and `1.2`. Pin a `sha-` or version tag in production, so
an upgrade is a choice you make and a rollback has somewhere to go.

The repository is public, so the packages are too. Check that once under the
repository's **Packages** after the first run. If a package is private,
add `ghcr.io` under **Registries** in Portainer with a GitHub token that has
`read:packages`.

The images are built for `linux/amd64` only.

## 2. Pick a stack file

| File | Use it when |
| --- | --- |
| [`portainer/stack.yaml`](portainer/stack.yaml) | Romeka faces the internet itself. Its Caddy takes ports 80 and 443 and gets the certificate from Let's Encrypt. |
| [`portainer/stack.proxy.yaml`](portainer/stack.proxy.yaml) | Something already holds 80 and 443 on the host: Nginx Proxy Manager, Traefik, another Caddy, a Cloudflare tunnel. Romeka serves plain HTTP on one port and the proxy in front does TLS. |

Both files keep everything in named volumes and mount nothing from a checkout,
so they work from the web editor.

## 3. Create the stack

1. In Portainer, open **Stacks > Add stack** and give it a name, for example
   `romeka`.
2. Choose one of the two build methods:
   - **Web editor**: paste the stack file.
   - **Repository**: URL `https://github.com/micahchoo/romeka-s`, reference
     `refs/heads/main`, compose path `deploy/portainer/stack.yaml` (or
     `stack.proxy.yaml`).
3. Under **Environment variables**, add the variables from section 4.
4. Click **Deploy the stack**.

The first start of an empty database installs it and creates the first
administrator. The container log says `romeka: /db/romeka.db is empty,
installing`. The container turns **healthy** within a minute.

Then open the base URL and sign in with the administrator email and password.

## 4. Environment variables

The required ones:

| Variable | `stack.yaml` | `stack.proxy.yaml` | Value |
| --- | --- | --- | --- |
| `ROMEKA_DOMAIN` | required | not used | the public host name, for example `collections.example.org` |
| `ROMEKA_BASE_URL` | required | required | the address in the browser's bar, for example `https://collections.example.org` |
| `ROMEKA_ACME_EMAIL` | required | not used | where certificate-expiry notices go |
| `ROMEKA_ADMIN_EMAIL` | first start | first start | the first administrator |
| `ROMEKA_ADMIN_NAME` | first start | first start | their display name |
| `ROMEKA_ADMIN_PASSWORD` | first start | first start | their password; the first start stops if it is empty |

`ROMEKA_BASE_URL` must be exactly the origin the browser uses, scheme and port
included. Sign-in fails if it is wrong, because the server refuses a
cookie-carrying write from any other origin.

Keep the three `ROMEKA_ADMIN_*` values set after the first start. An upgrade
(section 7) reads them again. It does not change the password of an account
that already exists.

The optional ones:

| Variable | Default | What it does |
| --- | --- | --- |
| `ROMEKA_IMAGE` | `ghcr.io/micahchoo/romeka-s:latest` | the image tag to run |
| `ROMEKA_HTTP_PORT` | `80`, or `8080` with `stack.proxy.yaml` | the host port for HTTP |
| `ROMEKA_HTTPS_PORT` | `443` | the host port for HTTPS, `stack.yaml` only |
| `ROMEKA_TRUSTED_PROXIES` | empty, or `private_ranges` with `stack.proxy.yaml` | the proxies whose `X-Forwarded-For` header is believed |
| `ROMEKA_MODE` | `serve` | `upgrade` for one start after a new image (section 7) |
| `ROMEKA_TITLE`, `ROMEKA_TIME_ZONE`, `ROMEKA_LOCALE` | `Romeka S`, `UTC`, `en` | written at install and at every upgrade |
| `ROMEKA_LOG` | `info` | `off`, `error`, `warn`, `info`, `debug` or `trace` |
| `ROMEKA_MAX_BODY_BYTES` | `268435456` | the largest request body |
| `ROMEKA_SMTP_*`, `ROMEKA_MAIL_FROM` | empty | mail; with no host, invitation links go to the log |
| `ROMEKA_STORE_BACKEND`, `ROMEKA_S3_*` | empty | store files in a bucket instead of the volume |

[`.env.example`](.env.example) explains each one in more detail. You can also
load it into Portainer with **Load variables from .env file** and then edit
the values. It sets `ROMEKA_IMAGE=romeka-s:local`, a tag that only exists
where the image was built, so change that line to a `ghcr.io` tag or delete
it.

## 5. Behind a proxy you already run

Use `stack.proxy.yaml`. The Romeka container still runs its own Caddy, on
plain HTTP. That Caddy serves the admin and checks permission on every media
file before serving it, so point the outer proxy at the container's port 80,
never at the server inside it.

Set up the outer proxy like this:

- **Target**: `http://<host>:8080`. If the proxy runs in Docker on the same
  host, put both on one network instead (the commented `networks` lines in the
  stack file) and use `http://romeka:80`.
- **Host header**: pass it through unchanged. Most proxies do this by default.
- **Body size**: allow uploads at least as large as `ROMEKA_MAX_BODY_BYTES`.
  In Nginx Proxy Manager, add `client_max_body_size 256m;` under the proxy
  host's **Advanced** tab.
- **Headers**: do not rewrite `Origin` or `Cookie`. Do send
  `X-Forwarded-For`.
- **TLS**: the outer proxy holds the certificate. `ROMEKA_BASE_URL` is its
  `https://` address.

`ROMEKA_TRUSTED_PROXIES` defaults to `private_ranges` in this file. That
trusts any proxy on a private address, which covers a Docker network and a
proxy on your LAN. With it, the login rate limit counts each visitor's own
address. If the outer proxy reaches Romeka from a public address, list that
address instead. Never trust a range that the public can reach directly,
because then anyone can forge the header.

## 6. Archive import and the office worker

Both are commented out in the stack files.

- **Archive import**: uncomment `ROMEKA_ARCHIVE_ROOT: /archive` and the
  `/archive` volume line, and change `/srv/archive` to the host folder to
  import from. The folder is mounted read-only.
- **Office worker**: uncomment the `office-worker` service. It makes the PDF
  copies of office documents and the RAW, 16-bit TIFF and PostScript copies
  the main image cannot. Give it the same tag as `ROMEKA_IMAGE`, under the
  `romeka-s-office-tools` name.

Then update the stack.

## 7. Upgrade

1. Take a backup (section 8).
2. In the stack's editor, set `ROMEKA_IMAGE` to the new tag and set
   `ROMEKA_MODE` to `upgrade`.
3. Click **Update the stack** with **Re-pull image** on.

   The container runs the schema migrations, then starts the server. The log
   says `romeka: upgrading /db/romeka.db, then serving`.
4. When the container is healthy, set `ROMEKA_MODE` back to `serve` and update
   the stack again.

Do not leave `ROMEKA_MODE` on `upgrade`. Every start in that mode rewrites the
title, time zone and locale from the environment, so a title you changed in
the admin would come back as `ROMEKA_TITLE` on each restart.

To roll back, set `ROMEKA_IMAGE` to the previous tag and restore the backup
from step 1. If the upgrade changed no schema, setting the tag back is enough.

## 8. Backup and restore

Portainer names the container `<stack>-romeka-1`, so `romeka-romeka-1` for a
stack called `romeka`. The commands below use that name.

### Back up

Open the container in Portainer, click **Console**, connect with
`/bin/bash`, and run:

```
mkdir -p /data/backups
romeka backup --database /db/romeka.db --store /data/files --out /data/backups/romeka-$(date -u +%Y%m%dT%H%M%SZ).tar
```

The server keeps serving while it runs. The archive lands in the files volume,
outside the media store. Copy it off the machine from a shell on the host,
then delete it from the volume:

```
docker cp romeka-romeka-1:/data/backups/romeka-20261024T120000Z.tar .
docker exec romeka-romeka-1 rm /data/backups/romeka-20261024T120000Z.tar
```

### Restore

A restore replaces the database and mirrors the store, so the server must be
stopped while it runs. Run these from a shell on the host:

```
docker cp romeka-20261024T120000Z.tar romeka-romeka-1:/db/.restore-archive.tar
docker stop romeka-romeka-1
docker run --rm --volumes-from romeka-romeka-1 --entrypoint romeka \
    ghcr.io/micahchoo/romeka-s:latest \
    restore --database /db/romeka.db --store /data/files --in /db/.restore-archive.tar --force
docker run --rm --volumes-from romeka-romeka-1 --entrypoint rm \
    ghcr.io/micahchoo/romeka-s:latest -f /db/.restore-archive.tar
docker start romeka-romeka-1
```

Use the same image tag the stack runs. The restore checks every file in the
archive against its manifest before it writes anything.

## 9. Volumes

Portainer prefixes each volume with the stack name:

| Volume | Holds |
| --- | --- |
| `<stack>_romeka-db` | the database |
| `<stack>_romeka-files` | the media store, and the backups from section 8 |
| `<stack>_romeka-caddy` | the certificate; losing it re-issues the certificate, and Let's Encrypt rate-limits that |

Deleting the stack keeps the volumes. Removing the volumes deletes the
deployment's data, so take a backup first.

## 10. When something is wrong

- **The container stops at once.** Read its log. A required variable that is
  missing is named there, for example `romeka: ROMEKA_BASE_URL is not set`.
- **The page loads but sign-in fails.** `ROMEKA_BASE_URL` does not match the
  address in the browser.
- **No certificate (`stack.yaml`).** The host name must resolve to this
  server, and ports 80 and 443 must reach it from the internet. The log names
  the reason; search it for `acme`.
- **Uploads fail behind a proxy.** The outer proxy's body limit is too small
  (section 5).

[`DEPLOY.md`](DEPLOY.md) covers mail, object storage, MySQL and the rest in
more depth. Its commands use `docker compose` from a checkout, but every
setting it names works the same in a Portainer stack.
