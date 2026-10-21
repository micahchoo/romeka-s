# Media-kind corpus

One small, valid file per media kind. `make.sh` writes all of them and
`make.sh --check` rebuilds them in a temp directory and compares every byte, so
the committed files are reproducible. Every file is under 64 KiB and the corpus
is under 1 MiB; `make.sh` fails when either bound breaks.

`crates/omeka-server/tests/media_matrix.rs` uploads every file through the public
API and records what the server made of it. `BASELINE.md` is the table the test
prints with `MEDIA_MATRIX_PRINT=1`.

| File | What it proves |
| --- | --- |
| `image.jpg` | JPEG ingest, derivative and EXIF Orientation 6: the upright large derivative is portrait, the unrotated one landscape. |
| `image.png` | PNG ingest and derivative. |
| `image.gif` | GIF ingest and derivative. |
| `image.webp` | WebP ingest and derivative. |
| `image.tif` | TIFF ingest and derivative, a decoder GD does not have. |
| `image-cmyk.tif` | CMYK TIFF: a layout the image crate refuses and `vips` turns into a JPEG access master. |
| `image-deep.tif` | 16-bit TIFF at 1200x800: a layout the image crate refuses, larger than the 800 pixel large derivative, so its canvas proves the master reaches the source's size. |
| `image-pages.tif` | Two-page TIFF: page one is the master. |
| `image.psd` | Photoshop document: `vips` reads it, the image crate does not. |
| `image.eps` | PostScript page: `ghostscript` rasterises page one. The bytes read as text, so the extension names the type. |
| `image.nef` | Nikon NEF ingest and display: the type named by the extension, and the three derivatives from the embedded `JpgFromRaw`, turned by the file's EXIF Orientation 6 so the large derivative is portrait. The embedded preview is 1200x800, larger than the 800 pixel derivative, so the access master's size proves deep zoom reaches the source. The file is a minimal NEF container (a maker TIFF with a JPEG SubIFD), not a whole camera file. |
| `image.avif` | AVIF ingest and display, outside upstream's default allow list. |
| `image.heic` | HEIC ingest and display. ImageMagick 7 reads HEIC but writes it read-only here, so `heif-enc` writes this file. |
| `image.svg` | Benign SVG ingest and display. |
| `hostile.svg` | SVG with `<script>`, `onload=`, a `javascript:` href and `<foreignObject>`; the `safe` cell proves none of it is reachable. |
| `audio.mp3` | MP3 audio page. |
| `audio.m4a` | AAC in MP4; libmagic names it `audio/x-m4a`, the server's alias makes it `audio/mp4`. |
| `audio.ogg` | Vorbis in Ogg. |
| `audio.opus` | Opus in Ogg; libmagic names the container `audio/ogg`, and the port reads the codec's answer as that container. |
| `audio.flac` | FLAC audio, outside upstream's default allow list. |
| `audio.wav` | PCM WAV. |
| `video.mp4` | H.264 and AAC video, the file a captions track is attached to. |
| `video.webm` | VP9 and Opus video. |
| `video.mov` | H.264 and AAC in QuickTime. |
| `video.mkv` | H.264 and AAC in Matroska, outside upstream's default allow list. |
| `video.ogv` | Theora and Vorbis in Ogg. The bytes read as the Ogg container, `audio/ogg`; the extension names the video type, `video/ogg`. |
| `doc.pdf` | Two-page PDF. |
| `doc.docx` | Word document. |
| `model.glb` | Binary glTF with an embedded buffer. |
| `model.gltf` | JSON glTF with a base64 data-URI buffer. |
| `captions.vtt` | WebVTT captions. |
| `captions.srt` | SubRip captions. |

## Regenerating

```
./make.sh          # rewrite the files in this directory
./make.sh --check  # prove the committed bytes are reproducible
```

The tools are `ffmpeg`, ImageMagick 7 (`magick`), `exiftool`, `heif-enc`,
`resvg` is not needed because the SVGs are written directly, and `python3` for
the PDF, DOCX, GLB and glTF containers. The PSD is ImageMagick's, because vips
reads a Photoshop document and writes none. The EPS is written as text, so no
tool is needed for it. The script strips container metadata and passes
`-bitexact` to ffmpeg where the muxer honours it.
