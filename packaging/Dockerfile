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
#
# The stages in order: the SPA, the Rust binary, the ffmpeg build, the media
# tools, Caddy with its low-port capability, and the runtime that joins them.
# `--target media-tools` builds the tools alone (packaging/README.md says why).

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
# The registry is a cache mount, so a dependency fetch is paid once. The
# compiled target is not: a cache mount at a fixed path is shared by every
# checkout that builds this image, and cargo compares mtimes, so a build here
# would happily reuse a crate another checkout compiled and fail on an API it
# does not have. `--locked` refuses a dependency change that Cargo.lock does not
# record.
#
# The CLI package's binary is installed under the name this image exports, so
# the crate's own binary name never has to match it. The loop holds the product's
# name and the name the crate declares on a tree that predates the rename, and
# installs whichever the build produced. Drop the second name and the loop once
# the rename has landed everywhere.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    set -eux; \
    cargo build --release --locked -p omeka-cli; \
    for bin in target/release/romeka target/release/omeka; do \
        if [ -x "$bin" ]; then install -D -m 0755 "$bin" /out/romeka; exit 0; fi; \
    done; \
    echo "no CLI binary at target/release" >&2; exit 1

# Stage 3: ffmpeg and ffprobe, statically linked.
#
# Debian 12's `ffmpeg` package installs 461 MiB of packages: libavfilter
# depends on Mesa's software GL (112 MiB of LLVM), PulseAudio, SDL, VA-API and
# VDPAU, none of which a still or a duration needs. Building only the native
# codecs against a static dav1d gives both commands in 38 MiB, and it decodes
# the HEIC still Debian 12's 5.1 cannot read (`packaging/README.md` has the
# measurement).
#
# Everything is pinned by URL and sha256. The build reads no network at run
# time (`--disable-network`): a tool opens a file in the store, never a URL.
FROM debian:bookworm-slim AS ffmpeg-build
ARG FFMPEG_VERSION=7.1.5
ARG FFMPEG_SHA256=de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f
ARG DAV1D_VERSION=1.5.1
ARG DAV1D_SHA256=401813f1f89fa8fd4295805aa5284d9aed9bc7fc1fdbe554af4292f64cbabe21
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl meson nasm ninja-build pkg-config xz-utils zlib1g-dev; \
    rm -rf /var/lib/apt/lists/*
WORKDIR /src
# dav1d is the AV1 decoder AVIF needs. FFmpeg's own AV1 decoder wants the
# sequence header out of band, and an AVIF keeps it in band, so without dav1d a
# frame from an AVIF fails with "No sequence header available".
RUN set -eux; \
    curl -fsSLo dav1d.tar.xz \
        "https://downloads.videolan.org/pub/videolan/dav1d/${DAV1D_VERSION}/dav1d-${DAV1D_VERSION}.tar.xz"; \
    echo "${DAV1D_SHA256}  dav1d.tar.xz" | sha256sum -c -; \
    mkdir dav1d; \
    tar -xJC dav1d --strip-components=1 -f dav1d.tar.xz; \
    meson setup dav1d/build dav1d \
        --buildtype=release --default-library=static --libdir=lib \
        -Denable_tools=false -Denable_tests=false --prefix=/opt/dav1d; \
    ninja -C dav1d/build install
# `--disable-network` also drops every protocol but a local file and a pipe,
# which is all the tools use. Wave U8's transcodes add their encoders to this
# configure line (`--enable-libx264` and an x264 build; AAC is native).
RUN set -eux; \
    curl -fsSLo ffmpeg.tar.xz "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"; \
    echo "${FFMPEG_SHA256}  ffmpeg.tar.xz" | sha256sum -c -; \
    mkdir ffmpeg; \
    tar -xJC ffmpeg --strip-components=1 -f ffmpeg.tar.xz; \
    cd ffmpeg; \
    PKG_CONFIG_PATH=/opt/dav1d/lib/pkgconfig ./configure \
        --prefix=/opt/ffmpeg \
        --disable-autodetect --enable-zlib --enable-libdav1d \
        --disable-doc --disable-debug --disable-shared --enable-static --enable-small \
        --disable-network --enable-protocol=file,pipe; \
    make -j"$(nproc)"; \
    install -D -m 0755 ffmpeg /opt/ffmpeg/bin/ffmpeg; \
    install -D -m 0755 ffprobe /opt/ffmpeg/bin/ffprobe

# Stage 4: the media tools. This stage is also the plan's separate tools image
# for the derivative jobs of wave U8 (`briefs/media-ui-plan.md`, open call 2):
#
#     docker build -f packaging/Dockerfile --target media-tools -t romeka-s:media-tools .
#
# Every command is optional at runtime (`crates/omeka-media/src/tool.rs`): an
# installation that drops one skips that derivative with the command's name as
# the reason, never an error. Each line names what asks for it.
FROM debian:bookworm-slim AS media-tools
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        libimage-exiftool-perl \
        poppler-utils; \
    rm -rf /var/lib/apt/lists/*
# `pdftoppm` (poppler-utils) makes a PDF's first page; `exiftool`
# (libimage-exiftool-perl) extracts the full-size JPEG a camera RAW file
# embeds, so a NEF gets derivatives without decoding its sensor data.
#
# Wave U8 adds vips (TIFF, PSD, EXR, HEIC), libraw and ghostscript to the apt
# line above. Measure the installed size of that addition before taking it.
COPY --from=ffmpeg-build /opt/ffmpeg /opt/ffmpeg
# `ffmpeg` makes a video's poster and decodes AVIF and HEIC, and its `ffprobe`
# reads the duration and frame size a time-based IIIF canvas needs.
ENV PATH=/opt/ffmpeg/bin:$PATH

# Stage 5: Caddy with its low-port capability set. The capability is an xattr
# on the binary and the runtime's COPY preserves it, so the 52 MiB binary is
# copied into the image once. Setting it in the runtime would copy the file up
# into a second layer of the same size.
#
# The official Caddy image is an Alpine build, and its binary is static, so it
# runs unchanged on the runtime's glibc base, which is what the Rust stage links
# against (glibc, not musl).
FROM caddy:2.11-alpine AS caddy
RUN apk add --no-cache libcap \
    && setcap cap_net_bind_service=+ep /usr/bin/caddy

# Stage 6: the runtime. It starts from the tools stage, so the tool list and
# their provenance above is the only place either is written down.
FROM media-tools AS runtime
COPY --from=caddy /usr/bin/caddy /usr/local/bin/caddy
# Caddy terminates TLS on :80 and :443 and the container runs as a non-root
# user, so the low-port capability is attached to the binary. A container that
# runs non-root gets no capability from `cap_add`, and the capability on the
# file is what lets a non-root process bind a privileged port.
#
# `media-types` installs /etc/mime.types, which Go's mime package reads at the
# first lookup. It is the floor under the server's stored type: a file with no
# row type still gets a correct Content-Type for .mov, .mkv, .ogv, .glb, .avif
# and the like, where the bare image had an empty table. `bash` is what the
# entrypoint needs (`wait -n`, arrays, `set -o pipefail`), and `curl` is what
# the health check and an operator's debugging reach for. `libcap2-bin` is
# `getcap`, which is how the capability on Caddy is read back from a running
# image.
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        bash ca-certificates curl libcap2-bin media-types; \
    rm -rf /var/lib/apt/lists/*; \
    groupadd --system romeka; \
    useradd --system --gid romeka --home-dir /data --shell /usr/sbin/nologin romeka; \
    mkdir -p /db /data/files /srv/dist /srv/caddy/data /srv/caddy/config /archive; \
    chown -R romeka:romeka /db /data /srv

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
