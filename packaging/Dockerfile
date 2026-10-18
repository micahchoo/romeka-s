# syntax=docker/dockerfile:1
#
# The Omeka S port as one service: the Rust release binary behind the admin
# bundle, with nginx as the static origin.
#
# Build from the repository root, not from this directory:
#
#     docker build -f packaging/Dockerfile -t omeka-s:local .
#
# The context is the whole workspace because the Rust stages compile it. The
# per-Dockerfile ignore file (packaging/Dockerfile.dockerignore) keeps the
# context small.

# Stage 1: the admin SPA bundle.
FROM node:22-bookworm-slim AS web
WORKDIR /web
# The lockfile drives the install, so a dependency change is the only thing
# that invalidates this layer.
COPY web/admin/package.json web/admin/package-lock.json ./
RUN npm ci
COPY web/admin/ ./
RUN npm run build

# Stage 2: the Rust release binary.
FROM rust:1.93-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# Cache mounts keep the registry and the compiled dependencies across rebuilds.
# `--locked` refuses a dependency change that Cargo.lock does not record.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p omeka-cli \
    && cp target/release/omeka /usr/local/bin/omeka

# Stage 3: the runtime. nginx is the static origin, so the image adds only curl
# for the health check.
FROM nginx:1.27-bookworm AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system omeka \
    && useradd --system --gid omeka --home-dir /data --shell /usr/sbin/nologin omeka \
    && mkdir -p /data/files /app/dist /etc/omeka \
    && chown -R omeka:omeka /data

COPY --from=build /usr/local/bin/omeka /usr/local/bin/omeka
COPY --from=web /web/dist /app/dist
COPY --chmod=0755 packaging/entrypoint.sh /usr/local/bin/omeka-entrypoint
COPY packaging/nginx.conf.template /etc/omeka/nginx.conf.template

ENV OMEKA_DB=/data/omeka.db \
    OMEKA_STORE=/data/files \
    OMEKA_DIST=/app/dist \
    OMEKA_API_HOST=127.0.0.1 \
    OMEKA_API_PORT=8123 \
    OMEKA_PUBLIC_PORT=8080 \
    OMEKA_NGINX_TEMPLATE=/etc/omeka/nginx.conf.template \
    OMEKA_NGINX_CONF=/tmp/nginx.conf

# The working directory is the data directory, because `omeka serve` resolves
# the media store as `files` relative to it. The store volume mounts at
# /data/files, so the server's delete path and the origin's served path agree.
WORKDIR /data

# The origin listens on 8080. The Rust port listens on 127.0.0.1:8123 inside
# the container and is not published.
EXPOSE 8080
USER omeka

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${OMEKA_PUBLIC_PORT}/health" || exit 1

ENTRYPOINT ["/usr/local/bin/omeka-entrypoint"]
