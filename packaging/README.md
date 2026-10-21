# packaging

The container image: the Romeka S server, and the Caddy origin in front of it
that terminates TLS and serves the admin SPA and the media bytes.

## Files

- `Dockerfile` builds seven stages. `web` builds the SPA, `build` builds the
  release binary of the `omeka-cli` package, `ffmpeg-build` compiles ffmpeg and
  ffprobe, `media-base` installs the commands every image carries, `media-tools`
  is the separate image with the access-copy commands the main one leaves out,
  `caddy` sets the low-port capability on the origin binary, and `runtime` joins
  the base tools with the SPA and the origin. Build it from the repository root:
  `docker build -f packaging/Dockerfile -t romeka-s:local .`
- `Dockerfile.dockerignore` is the build context filter.
- `entrypoint.sh` installs an absent database, starts the server on loopback and
  Caddy on 80 and 443, and stops the container when either process exits.

## The media tools

`media-base` is the command set every image carries; `media-tools` is the
separate image for the formats the main one leaves out. Every command is
optional at runtime (`crates/omeka-media/src/tool.rs`): an installation that
drops one skips that derivative with the command's name as the reason, never an
error (`needs <command>, not installed`).

| Command | Image | Source | Version | Why |
| --- | --- | --- | --- | --- |
| `ffmpeg`, `ffprobe` | both | built from source, `https://ffmpeg.org/releases/ffmpeg-7.1.5.tar.xz` sha256 `de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f` | 7.1.5 | a video's poster, an AVIF or HEIC frame, a duration and frame size |
| dav1d (linked into ffmpeg) | both | `https://downloads.videolan.org/pub/videolan/dav1d/1.5.1/dav1d-1.5.1.tar.xz` sha256 `401813f1f89fa8fd4295805aa5284d9aed9bc7fc1fdbe554af4292f64cbabe21` | 1.5.1 | the AV1 decoder an AVIF needs; FFmpeg's own decoder wants the sequence header out of band |
| `pdftoppm` | both | `poppler-utils` (Debian 12) | 22.12.0 | a PDF's first page |
| `exiftool` | both | `libimage-exiftool-perl` (Debian 12) | 12.57 | the full-size JPEG a camera RAW file embeds, which becomes its access master |
| `dcraw_emu` | `media-tools` | `libraw-bin` (Debian 12) | 0.20.2 | a camera RAW whose file embeds no preview |
| `vips` | `media-tools` | `libvips-tools` (Debian 12, with the TIFF, HEIC, PSD and EXR loaders) | 8.14.1 | the JPEG access master for a layout the `image` crate refuses |
| `gs` | `media-tools` | `ghostscript` (Debian 12) | 10.00.0 | the first page of an EPS, AI or PS file |

Both downloaded tarballs are checked against the sha256 above inside the build,
and the build reads no network at run time: ffmpeg is configured
`--disable-network`, so a tool opens a file in the store and never a URL.

**Why two images, measured 2026-10-21 on amd64.** Each command was installed on
its own into the `media-tools` stage and the image was weighed
(`docker image inspect -f '{{.Size}}'`, and its `RUN` layer in `docker history`):

| Variant of the tools stage | Image | Added over the base |
| --- | --- | --- |
| base (`exiftool`, `poppler-utils`, ffmpeg, CA certificates) | 217,092,445 B | 0 |
| no extra command, with the binary | 282,686,013 B | 0 |
| + `libraw-bin` | 286,710,758 B | 4.0 MB |
| + `libvips-tools` | 441,493,775 B | 158.8 MB |
| + `ghostscript` | 344,601,836 B | 61.9 MB |
| all three | 503,086,810 B | 220.4 MB |

`vips` is 158.8 MB, far over the "under about 60 MB" a main-image tool has to
clear, and `ghostscript` is 61.9 MB. `libraw` is 4.0 MB, but it cannot encode a
JPEG without `vips`, so it travels with `vips` rather than alone. The main image
therefore stays at **345,641,393 B** (it was 345 MB after `slim-image`): a NEF
still gets its full-size master through `exiftool`, and the odd TIFF, PSD, HEIC,
EXR and PostScript page wait for the tools image. A type with ffmpeg-made
thumbnails (HEIC, AVIF, BMP) is still a local image without `vips`; its canvas
is the large derivative.

`.evidence/tool-sizes.sh` rebuilds this table (`--build-arg
MEDIA_TOOLS_EXTRA=`), so the next command is measured the same way.

**How a job calls it.** The tools image is the same CLI with every command
installed, so it drains the same queue against the same database and store:

```
docker build -f packaging/Dockerfile --target media-tools -t romeka-s:media-tools .
docker run --rm \
    -v romeka_db:/db -v romeka_files:/data/files \
    romeka-s:media-tools \
    media derivatives --retry --database /db/romeka.db --store /data/files
```

`--retry` is what a split deployment needs: the server creates a media and its
access row with the commands the *server* has, so on the main image a 16-bit
TIFF is settled `none` with `needs vips, not installed`. The tools run re-queues
that row (the plan is re-read with `vips` present) and drains it. Two processes
claim from one queue safely: a claim is a conditional `UPDATE`, and each worker
claims only the commands its own install has. Run the command on a timer (the
compose service or a systemd timer, once a minute is plenty); the server's own
worker handles the `exiftool` rows as it always does.

## The ffmpeg build

Debian 12's `ffmpeg` package installs 461 MiB of packages, because `libavfilter`
depends on Mesa's software GL (112 MiB of LLVM), PulseAudio, SDL, VA-API and
VDPAU. A still and a duration need none of them. The source build keeps every
native decoder and demuxer (so an AVI, WMV, FLV or MPEG-2 poster still works),
adds dav1d for AV1, and produces both commands in 38 MiB.

It reads HEIC, which Debian 12's ffmpeg 5.1 cannot: measured on the corpus
(`fixtures/media-kinds/image.heic`), 5.1 fails with `moov atom not found` and
7.1.5 writes an 8644-byte PNG. The other stills are byte for byte what 5.1
produced.

The transcode encoders wave U8 needs are configure flags in the same stage.
H.264 arrives with libx264, which also makes the build GPL, and AAC is native.
That build is a call for the unit that adds the transcodes.

## Build caching

The Rust stage caches the cargo registry and the compiled target. A cache mount
at a fixed path belongs to the Docker daemon, not to the checkout, and cargo
decides freshness by mtime, so a build could reuse a crate another checkout
compiled. Measured on 2026-10-21: a deploy failed on fields its own `omeka-db`
had, and this unit's build failed with `missing field extension in initializer
of KindInput`, both after a build of a different checkout. The stage therefore
touches the copied sources before `cargo build`: the workspace crates rebuild
every time, and the dependencies stay cached.

## Measured size

`docker images` (disk usage column) and `docker history`, measured 2026-10-21
on amd64. Before is `romeka-s:05e3a30a`.

| Layer | Before | After |
| --- | --- | --- |
| base `debian:bookworm-slim` | 74.9 MB | 74.9 MB |
| apt: `ffmpeg` and every package it pulls | 548 MB, one layer shared with the `apt` rows below | gone |
| apt: `exiftool` and its Perl, `poppler-utils` | in the 548 MB | 96.9 MB |
| ffmpeg and ffprobe, built from source | none | 37.7 MB |
| apt: `bash`, `ca-certificates`, `curl`, `libcap2-bin`, `media-types` | in the 548 MB | 14.1 MB |
| Caddy, copied into the image | 52 MB | 52 MB |
| Caddy, copied up again by `setcap` | 52 MB | none |
| the romeka binary | 63 MB | 62.3 MB |
| the admin bundle | 5.1 MB | 5.1 MB |
| **total** | **795 MB** | **343 MB** |

The base tools stage (`media-base`, without the SPA, the origin or the binary) is
209 MB. The largest remaining item is the 62.3 MB Rust binary, and Caddy is
52 MB; a custom Caddy build with only the modules `deploy/Caddyfile` uses would
cut the second.

**After the access-copy commands (2026-10-21).** The runtime is 345,641,393 B,
unchanged: `exiftool`, `poppler-utils` and ffmpeg are still the whole set. The
tools image (`--target media-tools`) is 503,086,810 B: 220.4 MB of
`libvips-tools`, `libraw-bin` and `ghostscript`, 65.6 MB for the binary it now
carries, and the 217 MB base. Those numbers are the reason the main image keeps
its size and the heavy commands live in the second image (`packaging/README.md`,
"The media tools").

## Running it

`deploy/compose.yaml` runs this image with the database, the media store and the
certificates on volumes, and reads its settings from `deploy/.env`. That is the
supported way to run it, and `deploy/DEPLOY.md` is the runbook: first deployment,
TLS, mail, backup, restore, upgrade and rollback.

The image exposes 80 and 443 and nothing else. The server listens on `127.0.0.1`
inside the container, so the origin is the only way in.
