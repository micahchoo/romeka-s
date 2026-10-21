#!/usr/bin/env bash
# Generates the media-kind corpus. Run `./make.sh` to rewrite the files in this
# directory, or `./make.sh --check` to rebuild them in a temp directory and
# compare every byte, which is how the committed outputs are proven reproducible.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
mode=write
if [[ ${1:-} == --check ]]; then
  mode=check
  out=$(mktemp -d)
  trap 'rm -rf "$out" "$work"' EXIT
else
  out=$here
fi
work=$(mktemp -d)
[[ $mode == check ]] || trap 'rm -rf "$work"' EXIT

# A 120x80 asymmetric image: a red square top-left, a blue bar bottom-right and
# a yellow triangle off centre. A wrongly rotated derivative is visible.
magick -size 120x80 xc:'#f4f0e8' \
  -fill '#c0392b' -draw 'rectangle 0,0 46,38' \
  -fill '#2c3e50' -draw 'rectangle 73,41 119,79' \
  -fill '#f1c40f' -draw 'polygon 56,12 78,12 67,34' \
  -strip "$work/base.png"

magick "$work/base.png" -strip "$out/image.png"
magick "$work/base.png" -quality 82 -strip "$out/image.jpg"
# EXIF Orientation 6: the viewer must rotate the 120x80 pixels 90 degrees
# clockwise, which makes the upright derivative 80x120.
exiftool -q -overwrite_original -Orientation=6 -n "$out/image.jpg"
magick "$work/base.png" -strip "$out/image.gif"
magick "$work/base.png" -quality 80 -strip "$out/image.webp"
magick "$work/base.png" -compress zip -strip "$out/image.tif"
magick "$work/base.png" -quality 50 -strip "$out/image.avif"
heif-enc -q 50 -o "$out/image.heic" "$work/base.png" >/dev/null
# The preview a camera RAW embeds: the same 120x80 frame, JPEG, no metadata.
magick "$work/base.png" -quality 55 -strip "$work/nef-preview.jpg"

cat >"$out/image.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80" viewBox="0 0 120 80" role="img" aria-label="A red square and a blue bar">
  <rect width="120" height="80" fill="#f4f0e8"/>
  <rect width="47" height="39" fill="#c0392b"/>
  <rect x="73" y="41" width="47" height="39" fill="#2c3e50"/>
  <polygon points="56,12 78,12 67,34" fill="#f1c40f"/>
</svg>
SVG

cat >"$out/hostile.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80" viewBox="0 0 120 80" onload="alert('onload')">
  <script>alert('script')</script>
  <rect width="120" height="80" fill="#f4f0e8"/>
  <a href="javascript:alert('href')"><rect width="47" height="39" fill="#c0392b"/></a>
  <foreignObject x="60" y="10" width="50" height="30"><div xmlns="http://www.w3.org/1999/xhtml">foreign</div></foreignObject>
</svg>
SVG

sound() { # frequency, output, codec arguments...
  local frequency=$1 output=$2
  shift 2
  ffmpeg -nostdin -y -v error \
    -f lavfi -i "sine=frequency=$frequency:duration=0.35:sample_rate=8000" \
    -af volume=0.2 -ac 1 -map_metadata -1 -fflags +bitexact -flags:a +bitexact \
    "$@" "$out/$output"
}

sound 330 audio.mp3 -c:a libmp3lame -b:a 24k
sound 349 audio.m4a -c:a aac -b:a 24k -movflags +faststart
sound 392 audio.ogg -c:a libvorbis -q:a 0
sound 440 audio.opus -c:a libopus -b:a 16k -f ogg
sound 494 audio.flac -c:a flac -compression_level 8
sound 523 audio.wav -c:a pcm_s16le

video() { # output, video codec arguments, audio codec arguments
  local output=$1
  shift
  local -a video_args audio_args
  local seen_audio=0
  video_args=()
  audio_args=()
  for argument in "$@"; do
    if [[ $argument == -- ]]; then seen_audio=1; continue; fi
    if ((seen_audio)); then audio_args+=("$argument"); else video_args+=("$argument"); fi
  done
  ffmpeg -nostdin -y -v error \
    -f lavfi -i "testsrc2=size=96x64:rate=10:duration=0.5" \
    -f lavfi -i "sine=frequency=587:duration=0.5:sample_rate=8000" \
    -map 0:v -map 1:a -shortest -map_metadata -1 \
    -fflags +bitexact -flags +bitexact \
    "${video_args[@]}" "${audio_args[@]}" "$out/$output"
}

video video.mp4 -c:v libx264 -preset ultrafast -crf 34 -pix_fmt yuv420p -- -c:a aac -b:a 16k -movflags +faststart
video video.webm -c:v libvpx-vp9 -b:v 40k -crf 50 -pix_fmt yuv420p -- -c:a libopus -b:a 12k
video video.mov -c:v libx264 -preset ultrafast -crf 34 -pix_fmt yuv420p -f mov -- -c:a aac -b:a 16k
video video.mkv -c:v libx264 -preset ultrafast -crf 34 -pix_fmt yuv420p -f matroska -- -c:a aac -b:a 16k
video video.ogv -c:v libtheora -q:v 3 -pix_fmt yuv420p -f ogg -- -c:a libvorbis -q:a 0

cat >"$out/captions.vtt" <<'VTT'
WEBVTT

00:00:00.000 --> 00:00:00.400
A red square, then a blue bar.
VTT

cat >"$out/captions.srt" <<'SRT'
1
00:00:00,000 --> 00:00:00,400
A red square, then a blue bar.
SRT

python3 - "$out" <<'PY'
import struct
import sys
import zipfile
from base64 import b64encode
from pathlib import Path

out = Path(sys.argv[1])


def pdf() -> bytes:
    pages = ["Romeka media matrix", "Page two"]
    objects = {
        1: b"<< /Type /Catalog /Pages 2 0 R >>",
        2: b"<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Resources << /Font << /F1 4 0 R >> >> /Contents 6 0 R >>",
        4: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        5: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>",
    }
    for number, text in ((6, pages[0]), (7, pages[1])):
        stream = f"BT /F1 16 Tf 20 70 Td ({text}) Tj ET".encode()
        objects[number] = b"<< /Length %d >>\nstream\n%s\nendstream" % (len(stream), stream)
    document = bytearray(b"%PDF-1.4\n")
    offsets = {}
    for number in sorted(objects):
        offsets[number] = len(document)
        document += b"%d 0 obj\n%s\nendobj\n" % (number, objects[number])
    xref = len(document)
    document += b"xref\n0 %d\n" % (len(objects) + 1)
    document += b"0000000000 65535 f \n"
    for number in sorted(objects):
        document += b"%010d 00000 n \n" % offsets[number]
    document += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objects) + 1,
        xref,
    )
    return bytes(document)


def docx() -> bytes:
    content_types = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
        "</Types>"
    )
    relationships = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
        "</Relationships>"
    )
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        "<w:body><w:p><w:r><w:t>Romeka media matrix</w:t></w:r></w:p></w:body>"
        "</w:document>"
    )
    buffer = Path(out, ".docx.tmp")
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_STORED) as archive:
        for name, data in (
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", relationships),
            ("word/document.xml", document),
        ):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED
            archive.writestr(info, data)
    data = buffer.read_bytes()
    buffer.unlink()
    return data


def mesh() -> tuple[dict, bytes]:
    positions = struct.pack("<9f", 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0)
    indices = struct.pack("<3H2x", 0, 1, 2)
    binary = positions + indices
    document = {
        "asset": {"version": "2.0", "generator": "media-kinds corpus"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": len(positions)},
            {"buffer": 0, "byteOffset": len(positions), "byteLength": len(indices)},
        ],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 0.0],
            },
            {
                "bufferView": 1,
                "componentType": 5123,
                "count": 3,
                "type": "SCALAR",
            },
        ],
    }
    return document, binary


document, binary = mesh()

import json

gltf = json.loads(json.dumps(document))
gltf["buffers"][0]["uri"] = "data:application/octet-stream;base64," + b64encode(binary).decode()
Path(out, "model.gltf").write_text(json.dumps(gltf, separators=(",", ":"), sort_keys=True), "utf-8")

json_chunk = json.dumps(document, separators=(",", ":"), sort_keys=True).encode()
json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
bin_chunk = binary + b"\x00" * ((4 - len(binary) % 4) % 4)
glb = struct.pack(
    "<III", 0x46546C67, 2, 12 + 8 + len(json_chunk) + 8 + len(bin_chunk)
)
glb += struct.pack("<II", len(json_chunk), 0x4E4F534A) + json_chunk
glb += struct.pack("<II", len(bin_chunk), 0x004E4942) + bin_chunk
Path(out, "model.glb").write_bytes(glb)

Path(out, "doc.pdf").write_bytes(pdf())
Path(out, "doc.docx").write_bytes(docx())
PY

# A minimal Nikon NEF: the maker's TIFF container, its IFD0 naming the camera
# and Orientation 6, and one SubIFD holding a JpgFromRaw preview. A real
# BHC006 NEF keeps its full-size preview the same way (a JPEG-compressed
# SubIFD); this one has a 120x80 preview so the fixture stays under 64 KiB.
python3 - "$out" "$work/nef-preview.jpg" <<'PY'
import struct
import sys
from pathlib import Path

out, preview = Path(sys.argv[1]), Path(sys.argv[2]).read_bytes()
typesize = {1: 1, 2: 1, 3: 2, 4: 4}
entry_size = 12


def ifd(entries, offset):
    entries = sorted(entries)
    body = 2 + entry_size * len(entries) + 4
    data = bytearray()
    encoded = bytearray(struct.pack("<H", len(entries)))
    for tag, kind, count, value in entries:
        encoded += struct.pack("<HHI", tag, kind, count)
        if typesize[kind] * count <= 4:
            encoded += value.ljust(4, b"\0")[:4]
        else:
            encoded += struct.pack("<I", offset + body + len(data))
            data += value
            if len(data) % 2:
                data += b"\0"
    encoded += struct.pack("<I", 0)
    return bytes(encoded) + bytes(data)


def ascii_entry(tag, text):
    value = text.encode() + b"\0"
    return tag, 2, len(value), value


def short(tag, *values):
    return tag, 3, len(values), struct.pack("<" + "H" * len(values), *values)


def long(tag, *values):
    return tag, 4, len(values), struct.pack("<" + "I" * len(values), *values)


sub_offset = jpeg_offset = 0
for _ in range(3):
    ifd0 = [
        long(256, 120),
        long(257, 80),
        short(258, 8, 8, 8),
        short(259, 1),
        short(262, 2),
        ascii_entry(271, "NIKON CORPORATION"),
        ascii_entry(272, "NIKON D3300"),
        short(274, 6),
        short(277, 3),
        short(284, 1),
        long(330, sub_offset),
    ]
    ifd0_bytes = ifd(ifd0, 8)
    sub_offset = 8 + len(ifd0_bytes)
    sub = [
        long(254, 1),
        long(256, 120),
        long(257, 80),
        short(258, 8, 8, 8),
        short(259, 6),
        short(262, 6),
        short(277, 3),
        long(278, 80),
        short(530, 2, 1),
        long(513, jpeg_offset),
        long(514, len(preview)),
        long(273, jpeg_offset),
        long(279, len(preview)),
    ]
    sub_bytes = ifd(sub, sub_offset)
    jpeg_offset = sub_offset + len(sub_bytes)

nef = b"II*\0" + struct.pack("<I", 8) + ifd(ifd0, 8) + ifd(sub, sub_offset) + preview
Path(out, "image.nef").write_bytes(nef)
PY

rm -f "$out/.docx.tmp"

python3 - "$out" <<'PY'
import hashlib
import sys
from pathlib import Path

out = Path(sys.argv[1])
limit = 64 * 1024
total = 0
named = 0
for path in sorted(out.iterdir()):
    if path.name in {"make.sh", "README.md", "BASELINE.md"} or path.name.startswith("."):
        continue
    named += 1
    size = path.stat().st_size
    total += size
    if size >= limit:
        raise SystemExit(f"{path.name}: {size} bytes, the limit is under 64 KiB")
    print(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {size:>6}  {path.name}")
if total >= 1024 * 1024:
    raise SystemExit(f"corpus is {total} bytes, the limit is under 1 MiB")
print(f"{named} files, {total} bytes")
PY

if [[ $mode == check ]]; then
  for file in "$out"/*; do
    name=$(basename "$file")
    cmp -s "$file" "$here/$name" || { echo "not reproducible: $name" >&2; exit 1; }
  done
  echo "reproducible: every byte matches the committed corpus"
fi
