# syntax=docker/dockerfile:1
#
# Romeka S in one image: the server, and the Caddy origin that terminates TLS,
# serves the admin SPA and serves the media bytes. The server itself has routes
# for neither (`crates/omeka-server/src/lib.rs` mounts /api, /api-context,
# /assets, /s, /health and /ready, and everything else is a 404), so the origin
# is what a browser talks to.
#
# Build from the repository root, not from this directory:
#
#     docker build -f packaging/Dockerfile -t romeka-s:local .
#
# The context is the whole workspace because the Rust and SPA stages compile it.
# packaging/Dockerfile.dockerignore keeps the context small.

# Stage 1: the admin SPA bundle.
FROM node:22-bookworm-slim AS web
WORKDIR /web
# The lockfile drives the install, so only a dependency change invalidates this
# layer. `npm ci` refuses a package.json that disagrees with the lockfile.
COPY web/admin/package.json web/admin/package-lock.json ./
RUN npm ci
COPY web/admin/ ./
# `npm run build` is `tsc --noEmit && vite build`, so a type error fails the
# image rather than shipping.
RUN npm run build

# Stage 2: the Rust release binary.
FROM rust:1.93-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# Cache mounts keep the registry and the compiled dependencies across rebuilds.
# `--locked` refuses a dependency change that Cargo.lock does not record. The
# binary is copied out in this same step: a cache mount is not visible to the
# next one, so an artifact left in target/ would not reach it.
#
# The CLI package's binary is installed under the name this image exports, so
# the crate's own binary name never has to match it. The loop holds the product's
# name and the name the crate declares on a tree that predates the rename, and
# installs whichever the build produced. Drop the second name and the loop once
# the rename has landed everywhere.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    set -eux; \
    cargo build --release --locked -p omeka-cli; \
    for bin in target/release/romeka target/release/omeka; do \
        if [ -x "$bin" ]; then install -D -m 0755 "$bin" /out/romeka; exit 0; fi; \
    done; \
    echo "no CLI binary at target/release" >&2; exit 1

# Stage 3: the runtime. Caddy is the second process; curl is what the health
# check and an operator's debugging reach for.
FROM debian:bookworm-slim AS runtime
# The official Caddy image is an Alpine build, and its binary is static, so it
# runs unchanged on this glibc base. Copying the one binary keeps the runtime
# stage bookworm, which is what the Rust stage links against (glibc, not musl).
COPY --from=caddy:2.11-alpine /usr/bin/caddy /usr/local/bin/caddy
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash ca-certificates curl ffmpeg libcap2-bin media-types poppler-utils \
    && rm -rf /var/lib/apt/lists/*
# `media-types` installs /etc/mime.types, which Go's mime package reads at the
# first lookup. It is the floor under the server's stored type: a file with no
# row type still gets a correct Content-Type for .mov, .mkv, .ogv, .glb, .avif
# and the like, where the bare image had an empty table.
#
# `ffmpeg` makes a video's poster and decodes AVIF and HEIC; `poppler-utils`
# is `pdftoppm`, which makes a PDF's first page. Both are optional at runtime
# (`crates/omeka-media/src/tool.rs`): an installation that drops them skips
# those derivatives with the command's name as the reason, so an operator who
# needs a smaller image removes two package names and nothing else breaks.

# Caddy terminates TLS on :80 and :443 and the container runs as a non-root user,
# so the low-port capability is attached to the binary. A container that runs
# non-root gets no capability from `cap_add`, and the capability on the file is
# what lets a non-root process bind a privileged port.
RUN setcap cap_net_bind_service=+ep /usr/local/bin/caddy \
    && groupadd --system romeka \
    && useradd --system --gid romeka --home-dir /data --shell /usr/sbin/nologin romeka \
    && mkdir -p /db /data/files /srv/dist /srv/caddy/data /srv/caddy/config /archive \
    && chown -R romeka:romeka /db /data /srv

COPY --from=build /out/romeka /usr/local/bin/romeka
COPY --from=web /web/dist /srv/dist
# The image carries the origin config so a plain `docker run` works; compose
# mounts the same file from deploy/ so an operator edit needs a restart.
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY --chmod=0755 packaging/entrypoint.sh /usr/local/bin/romeka-entrypoint

# ROMEKA_DB and ROMEKA_STORE are the deployment's, not the server's: compose
# mounts a volume at each path. ROMEKA_API_HOST and ROMEKA_API_PORT are the
# server's own variable names, and the entrypoint passes them on the command
# line, which wins over the environment, so the two agree by construction.
ENV ROMEKA_DB=/db/romeka.db \
    ROMEKA_STORE=/data/files \
    ROMEKA_DIST=/srv/dist \
    ROMEKA_CADDYFILE=/etc/caddy/Caddyfile \
    ROMEKA_API_HOST=127.0.0.1 \
    ROMEKA_API_PORT=8123 \
    XDG_DATA_HOME=/srv/caddy/data \
    XDG_CONFIG_HOME=/srv/caddy/config

# ROMEKA_ARCHIVE_ROOT is unset, so the archive import is off. To turn it on,
# mount a folder read-only at /archive and set ROMEKA_ARCHIVE_ROOT=/archive
# (deploy/compose.archive.yaml does both).

# The store root the server resolves is `files` under its working directory
# (`crates/omeka-server/src/lib.rs`, FILES_ROOT), so the process starts in the
# store's parent and the volume mount agrees with the path it serves.
WORKDIR /data

# The origin listens on 80 and 443 and the server on loopback, so only the two
# public ports leave the container.
EXPOSE 80 443

USER romeka

# Liveness: the server answers. It goes to the server directly rather than
# through Caddy, so an origin that is still waiting for a certificate cannot
# make a healthy server look dead. compose reads this same check.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -fsS "http://${ROMEKA_API_HOST}:${ROMEKA_API_PORT}/health" || exit 1

ENTRYPOINT ["/usr/local/bin/romeka-entrypoint"]
