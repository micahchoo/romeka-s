# Thumbnail fixtures

`gradient-96x64.jp2` (463 bytes) is the one JPEG 2000 original the thumbnail
tests read. No pure-Rust JPEG 2000 encoder exists, so it cannot be made in the
test. It was made with ImageMagick and OpenJPEG:

```sh
magick -size 96x64 gradient:'#2060a0-#e0c040' -depth 8 gradient.ppm
opj_compress -i gradient.ppm -o gradient-96x64.jp2 -r 40 -n 4
```
