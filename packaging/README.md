# packaging

The container image: the Romeka S server, and the Caddy origin in front of it
that terminates TLS and serves the admin SPA and the media bytes.

## Files

- `Dockerfile` builds seven stages. `web` builds the SPA, `build` builds the
  release binary of the `omeka-cli` package, `ffmpeg-build` compiles ffmpeg and
  ffprobe, `media-tools` installs the external media commands, `office-tools`
  adds LibreOffice and the job runner to those commands as a separate image,
  `caddy` sets the low-port capability on the origin binary, and `runtime` joins
  the SPA, the binary, the media tools and Caddy. Build it from the repository
  root: `docker build -f packaging/Dockerfile -t romeka-s:local .`
- `Dockerfile.dockerignore` is the build context filter.
- `entrypoint.sh` installs an absent database, starts the server on loopback and
  Caddy on 80 and 443, and stops the container when either process exits.

## The media tools

`media-tools` is where the commands the media pipeline may run are listed. Every
one is optional at runtime (`crates/omeka-media/src/tool.rs`): an installation
that drops one skips that derivative with the command's name as the reason,
never an error.

| Command | Source | Version | Why |
| --- | --- | --- | --- |
| `ffmpeg`, `ffprobe` | built from source, `https://ffmpeg.org/releases/ffmpeg-7.1.5.tar.xz` sha256 `de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f` | 7.1.5 | a video's poster, an AVIF or HEIC frame, a duration and frame size |
| dav1d (linked into ffmpeg) | `https://downloads.videolan.org/pub/videolan/dav1d/1.5.1/dav1d-1.5.1.tar.xz` sha256 `401813f1f89fa8fd4295805aa5284d9aed9bc7fc1fdbe554af4292f64cbabe21` | 1.5.1 | the AV1 decoder an AVIF needs; FFmpeg's own decoder wants the sequence header out of band |
| `pdftoppm` | `poppler-utils` (Debian 12) | 22.12.0 | a PDF's first page |
| `exiftool` | `libimage-exiftool-perl` (Debian 12) | 12.57 | the full-size JPEG a camera RAW file embeds |

Both downloaded tarballs are checked against the sha256 above inside the build,
and the build reads no network at run time: ffmpeg is configured
`--disable-network`, so a tool opens a file in the store and never a URL.

The whole tools set is one stage, so the U8 plan's separate tools image is one
target away (`briefs/media-ui-plan.md`, open call 2):

```
docker build -f packaging/Dockerfile --target media-tools -t romeka-s:media-tools .
```

That image holds the commands and nothing else. Wave U8 adds vips, libraw and
ghostscript to the `apt-get install` line in that stage; measure the addition
before taking it.

## The office document tools

An office document gets a PDF access copy through LibreOffice
(`crates/omeka-media/src/access.rs`, `D6` of `briefs/media-ui-plan.md`).
`soffice` is optional at runtime like the other tools: where it is absent the
row records `needs soffice, not installed` and the media keeps its card.

It is a separate image, not part of `runtime`, because of its size. Measured
2026-10-21 on amd64 with Debian 12's packages:

| Layer | Size |
| --- | --- |
| `libreoffice-writer-nogui` 7.4.7, `libreoffice-calc-nogui`, `libreoffice-impress-nogui` and everything they pull | 391 MB |
| `fonts-dejavu` (2.37) and `fonts-liberation` (1.07.4) | 10 MB |
| **added to the tools image** | **401 MB** |
| the tools image with them, plus the CLI | 676 MB |
| the tools image without them (`--target media-tools`) | 209 MB |
| the runtime image, which is unchanged | 343 MB |

The `-nogui` packages are LibreOffice's headless builds and need no X server.
The fonts are what a document names; without them the PDF substitutes another
face and the line breaks move. Build and run it against the server's database
and store:

```
docker build -f packaging/Dockerfile --target office-tools -t romeka-s:office-tools .
docker run --rm -v romeka-db:/db -v romeka-files:/data/files \
    romeka-s:office-tools romeka media derivatives --retry
```

`--retry` puts back the rows that recorded `needs soffice, not installed` when
the server planned them. The image carries the same CLI as the runtime, so it
is the job runner in that container; the server itself never needs LibreOffice.

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

The tools image alone (`--target media-tools`) is 209 MB. The largest remaining
item is the 62.3 MB Rust binary, and Caddy is 52 MB; a custom Caddy build with
only the modules `deploy/Caddyfile` uses would cut the second.

## Running it

`deploy/compose.yaml` runs this image with the database, the media store and the
certificates on volumes, and reads its settings from `deploy/.env`. That is the
supported way to run it, and `deploy/DEPLOY.md` is the runbook: first deployment,
TLS, mail, backup, restore, upgrade and rollback.

The image exposes 80 and 443 and nothing else. The server listens on `127.0.0.1`
inside the container, so the origin is the only way in.
