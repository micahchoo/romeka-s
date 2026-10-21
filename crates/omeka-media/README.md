# omeka-media

Romeka S is a fork of Omeka S. This crate holds the media subsystem: the `Store` trait every stored byte goes through,
its local implementation `FileStore` and the storage path rule, one ingester and one renderer per upstream class, the
file renderers, the three thumbnail types, the `media.data` payloads, the oEmbed service, the access-copy table
(`access`), the JPEG master it names (`access_master`), the transcode that makes
an audio, video or office document access copy (`transcode`), and `iiif_image`,
the IIIF Image API at level 1 over a stored image (a Romeka addition: its parse
and plan are pure, the server routes it). `src/lib.rs` documents the module map
and the one network seam (`http::HttpFetch`), and each ingester, renderer and
thumbnailer names the upstream PHP class it mirrors.

The store has six prefixes: `original`, `asset`, the three thumbnail types, and `access`, a Romeka-only one for the
playable or viewable copy of an original. A copy is never the original: `access/<storage id>.mp4` or
`access/<storage id>.jpg` sits beside `original/<storage id>.avi`, and the original stays downloadable and checksummed.

## Benchmarks

`benches/` holds one criterion target per area. Each is a `harness = false` binary that `criterion_main!` drives.

| target | area | source |
| --- | --- | --- |
| `store_paths` | the storage path rule, the URL and local path, the storage id, the checksum, the four filesystem calls | `src/store/` |
| `ingest_parse` | one case per ingester, plus the two boundaries around it | `src/ingest/`, `src/data.rs` |
| `render_html` | the six media renderers, the file renderer lookup, the text helpers | `src/render/`, `src/file_render/`, `src/text.rs` |
| `thumbnail` | decode, the three resizes, the JPEG encode, the store path, the generate-and-store path | `src/thumbnailer.rs` |

Run one target, all of them, or a filtered set:

```sh
cargo bench -p omeka-media
cargo bench -p omeka-media --bench thumbnail
cargo bench -p omeka-media -- thumbnail/derivative_bytes
```

Criterion always uses the `bench` profile, which inherits `release`. A debug build makes every number here meaningless.
Point `CARGO_TARGET_DIR` at the big disk before a run: the stands that write files root their scratch trees under
`$CARGO_TARGET_DIR/bench-tmp` (`benches/support/mod.rs`), never under `/tmp`, whose quota a full run exhausts and whose
failure arrives as a store error that looks like a bug in `FileStore`.

The ingester benches use the crate's own test doubles (`testing::FakeHttp`, `FakeThumbnailer`, `PassthroughPurifier`),
so no case touches the wire. Each one runs its case once before timing and asserts the work happened: the file is on
disk, the thumbnail flag is set, the fetch count is what the fixture needs. `cargo bench -- --test` runs every bench
once and checks those guards.

## Comparing against the baseline

Two ways, for two different jobs.

For a comparison, let criterion do it. It compares distributions and prints a p-value per bench:

```sh
cargo bench -p omeka-media -- --save-baseline before
# change the code
cargo bench -p omeka-media -- --baseline before
```

`benches/baseline.json` is the record of the last full run: for each bench its median, its 95% confidence interval for
the median, its standard deviation, its sample count and its total timed iterations, plus the commit, the rustc version,
the core count and the load average. Regenerate it with:

```sh
./benches/collect_baseline.sh                 # every group
./benches/collect_baseline.sh thumbnail       # criterion filters pass through
```

The script clears the previous `new` trees out of the criterion directory first, because criterion keeps every crate's
results in one place and the file must hold this crate's benches only. The file is flat JSON keyed `group/bench`, so
`jq` or `git diff` reads it without tooling.

## The numbers

One session, `bench` profile, 32 cores, rustc 1.93.1. The load average was 5.0 at the start of this run and 7.1 at the
end, on a box six agents share. Each row is a median of 50 samples (30 for the thumbnail transform) with criterion's
95% confidence interval for the median. Iterations is the total number of timed units criterion executed; for the
benches that take a fresh case per unit (`ingest/*`, `put/*`, `generate_thumbnails/*`) the setup runs outside the timed
region. One row is later than the rest: `media_render/html` was re-measured after the HTML renderer began purifying the
stored fragment, on a much busier box (load average 22), so its absolute value is an upper bound. The store rows
predate the `Store` trait: `put` now writes a sibling and renames it, as `put_atomic` did, `put_atomic` is gone, and
`uri` is now the `url` bench, so `put_1KiB`, `put_atomic_1KiB` and `uri` wait for a re-measure.

| bench | median | 95% CI | sd | samples | iterations | limiter |
| --- | --- | --- | --- | --- | --- | --- |
| `media_ingest_parse/html` | 6.312 us | 6.253 us to 6.439 us | 994.77 ns | 50 | 5100 | the purifier passthrough and one `String` into the payload; no I/O |
| `media_ingest_parse/iiif` | 75.200 us | 74.617 us to 76.359 us | 241.668 us | 50 | 21675 | the info-document fetch and parse, the size pick, the thumbnail fetch and the writes |
| `media_ingest_parse/iiif_presentation` | 308.205 us | 305.579 us to 310.389 us | 28.337 us | 50 | 3825 | the 99 KB manifest's JSON parse (184 us) and its clone into the payload (79 us) are 86% of it, both measured directly |
| `media_ingest_parse/oembed` | 73.559 us | 73.003 us to 75.112 us | 101.774 us | 50 | 20400 | the JSON response fetch, the thumbnail fetch, then the writes |
| `media_ingest_parse/upload` | 104.683 us | 103.430 us to 117.352 us | 97.849 us | 50 | 15300 | four store writes (the 8 KiB original and three fake thumbnails) and one SHA-256; the store's own cost is not stable enough to split (see `put/1KiB`), so the write share is inconclusive |
| `media_ingest_parse/url` | 90.217 us | 88.507 us to 91.268 us | 3.861 us | 50 | 20400 | the fake fetch (a `BTreeMap` lookup) plus one original write and three thumbnail writes |
| `media_ingest_parse/youtube` | 70.369 us | 68.923 us to 72.803 us | 173.942 us | 50 | 22950 | the poster fetch plus the original and three thumbnail writes |
| `media_ingest_parse_boundary/ingest_request_from_value_html` | 38.16 ns | 37.99 ns to 38.30 ns | 0.71 ns | 50 | 54089325 | a `Map` clone plus the request wrapper, on already parsed JSON |
| `media_ingest_parse_boundary/ingest_request_from_value_upload` | 30.07 ns | 29.99 ns to 30.11 ns | 0.34 ns | 50 | 69015750 | the same clone for a one-key body |
| `media_ingest_parse_boundary/media_data_parse_html` | 14.06 ns | 13.97 ns to 14.12 ns | 0.16 ns | 50 | 153665550 | one `get`, one `as_str`, one `String` |
| `media_ingest_parse_boundary/media_data_parse_iiif_json_clone_99KiB` | 79.412 us | 78.590 us to 80.179 us | 1.229 us | 50 | 25500 | one deep clone of the parsed 99 KB tree |
| `media_ingest_parse_boundary/media_data_parse_iiif_json_parse_99KiB` | 184.316 us | 183.801 us to 185.267 us | 1.792 us | 50 | 11475 | `serde_json::from_slice` of the 99 KB manifest, the ingest's own parse cost |
| `media_ingest_parse_boundary/media_data_parse_iiif_presentation` | 76.496 us | 76.302 us to 76.715 us | 870.00 ns | 50 | 26775 | the whole manifest tree cloned into the enum |
| `media_ingest_parse_boundary/media_data_parse_none` | 2.68 ns | 2.66 ns to 2.68 ns | 0.04 ns | 50 | 750556800 | the null filter and an enum unit |
| `media_ingest_parse_boundary/media_data_parse_oembed` | 258.79 ns | 257.79 ns to 262.01 ns | 26.74 ns | 50 | 7688250 | a `Map` clone of the response object |
| `media_render/file_thumbnail` | 507.88 ns | 506.02 ns to 510.19 ns | 11.45 ns | 50 | 3904050 | the media-type miss, the extension miss, then the thumbnail renderer and its `<img>` assembly |
| `media_render/file_video` | 454.86 ns | 454.16 ns to 456.61 ns | 8.21 ns | 50 | 4379625 | the media-type alias lookup, then the video renderer's `format!` |
| `media_render/html` | 7.195 us | 7.190 us to 7.197 us | 16.61 ns | 50 | 276675 | the purifier re-parse of the stored fragment, now the whole cost |
| `media_render/html_fulltext` | 355.64 ns | 311.00 ns to 404.61 ns | 110.82 ns | 50 | 8522100 | `strip_tags` then `html_entity_decode` over the stored fragment |
| `media_render/iiif` | 4.300 us | 4.238 us to 4.475 us | 733.36 ns | 50 | 473025 | the info document is re-serialised with sorted keys (a `BTreeMap` build) and escaped; 4.30 us against 0.67 us for a plain `to_string` of the same document |
| `media_render/iiif_document_serialize` | 665.17 ns | 654.07 ns to 677.21 ns | 184.33 ns | 50 | 2785875 | a plain `to_string` of the 1 KB info document, the floor under the iiif renderer |
| `media_render/iiif_presentation` | 1.140 us | 1.132 us to 1.153 us | 313.62 ns | 50 | 967725 | `urlencode` of the source and the config, then the iframe `format!` |
| `media_render/media_view_new_file` | 429.33 ns | 428.62 ns to 429.82 ns | 5.77 ns | 50 | 4644825 | three thumbnail URLs and a title, each a `format!` |
| `media_render/oembed` | 286.42 ns | 285.93 ns to 287.15 ns | 2.98 ns | 50 | 6948750 | the response `html` clone |
| `media_render/unknown_name_fallback` | 20.15 ns | 20.09 ns to 20.34 ns | 0.25 ns | 50 | 97272300 | a miss allocates `Arc::new(Fallback)` per call; 20.0 ns against 16.1 ns for a known lookup |
| `media_render/youtube` | 574.80 ns | 572.57 ns to 579.61 ns | 6.87 ns | 50 | 3492225 | the iframe `format!` and the escaped title |
| `media_renderer_lookup/file_renderer_by_extension` | 13.46 ns | 13.43 ns to 13.49 ns | 0.13 ns | 50 | 148115475 | the same two lookups |
| `media_renderer_lookup/file_renderer_by_media_type` | 14.38 ns | 14.35 ns to 14.41 ns | 0.33 ns | 50 | 138203625 | a `BTreeMap` lookup in the alias table then the renderer table |
| `media_renderer_lookup/file_renderer_unknown` | 10.05 ns | 10.04 ns to 10.10 ns | 0.11 ns | 50 | 198382350 | the alias lookup and the renderer lookup, both missing |
| `media_renderer_lookup/media_renderer_known` | 16.13 ns | 16.10 ns to 16.15 ns | 0.08 ns | 50 | 123940200 | a `BTreeMap` lookup, a handful of `&str` comparisons |
| `media_renderer_lookup/media_renderer_unknown` | 19.20 ns | 19.19 ns to 19.23 ns | 0.16 ns | 50 | 104126700 | the miss path, including the `Arc` allocation |
| `media_store_paths/delete_1KiB` | 3.342 us | 3.263 us to 3.434 us | 370.74 ns | 50 | 160650 | an `exists` stat plus the `unlink`, a directory-metadata write; stable across five sessions (3.34 to 3.83 us) |
| `media_store_paths/exists_missing` | 325.84 ns | 325.06 ns to 326.36 ns | 2.39 ns | 50 | 5859900 | one `stat`; 106 ns faster than the present case, not explained |
| `media_store_paths/exists_present` | 473.73 ns | 442.75 ns to 569.08 ns | 161.04 ns | 50 | 4573425 | one `stat` |
| `media_store_paths/extension_from_source_name_long_extension` | 4.78 ns | 4.74 ns to 4.84 ns | 0.10 ns | 50 | 405562200 | rejected on the 12-byte length rule |
| `media_store_paths/extension_from_source_name_no_dot` | 5.16 ns | 5.01 ns to 5.18 ns | 0.18 ns | 50 | 394585725 | `rfind` returns `None` |
| `media_store_paths/extension_from_source_name_trailing_dot` | 2.89 ns | 2.89 ns to 2.89 ns | 0.03 ns | 50 | 692702400 | rejected on the empty extension |
| `media_store_paths/extension_from_source_name_two_dots` | 11.00 ns | 10.99 ns to 11.02 ns | 0.10 ns | 50 | 181563825 | `rfind` finds the last dot |
| `media_store_paths/extension_from_source_name_upper_case` | 11.85 ns | 11.84 ns to 11.86 ns | 0.07 ns | 50 | 169622175 | `rfind('.')` then a lowercase copy |
| `media_store_paths/local_path_ok` | 50.16 ns | 50.12 ns to 50.22 ns | 0.31 ns | 50 | 39717525 | `has_parent_component` splits the path, then `PathBuf::join` allocates |
| `media_store_paths/local_path_parent_component` | 12.38 ns | 12.35 ns to 12.41 ns | 0.12 ns | 50 | 160470225 | rejected during the split scan, no allocation |
| `media_store_paths/new_storage_id` | 562.84 ns | 561.63 ns to 563.63 ns | 5.62 ns | 50 | 3510075 | one `getrandom` call plus the 40-char hex encode |
| `media_store_paths/put_1KiB` | 81.019 us | 25.955 us to 156.028 us | 168.433 us | 50 | 7650 | file syscalls (a `create_dir_all` stat, open, write, close); inconclusive: the 95% CI spans 25.9 to 156.0 us in this session and the median spanned 24.7 us to 2.86 ms over six, see the checklist |
| `media_store_paths/put_atomic_1KiB` | 45.598 us | 24.600 us to 84.811 us | 213.001 us | 50 | 14025 | the same syscalls plus a write to a temp sibling and a rename; inconclusive for the same reason (24.6 to 84.8 us here) |
| `media_store_paths/read_1KiB` | 1.061 us | 1.052 us to 1.070 us | 18.82 ns | 50 | 1889550 | open, read, close |
| `media_store_paths/sha256_hex_1KiB` | 1.163 us | 1.160 us to 1.166 us | 9.97 ns | 50 | 1728900 | 2.5 GB/s of hashing (0.41 us of the 1.16 us); the rest is digest setup and the hex encode |
| `media_store_paths/sha256_hex_1MiB` | 423.812 us | 423.654 us to 423.930 us | 467.70 ns | 50 | 5100 | SHA-256 at 2.47 GB/s, CPU-bound, no I/O in the path |
| `media_store_paths/storage_path_original_with_extension` | 62.95 ns | 62.40 ns to 63.56 ns | 5.29 ns | 50 | 31445325 | one `format!` allocation |
| `media_store_paths/storage_path_thumbnail_without_extension` | 30.60 ns | 29.97 ns to 31.41 ns | 5.06 ns | 50 | 51823650 | the same `format!`, no extension arm |
| `media_store_paths/uri` | 48.55 ns | 48.45 ns to 48.65 ns | 0.79 ns | 50 | 41117475 | one `format!` allocation |
| `media_text/escape_html_fragment` | 306.23 ns | 305.28 ns to 307.35 ns | 4.37 ns | 50 | 6633825 | one pass over the 400-byte fragment, output `String` |
| `media_text/escape_html_short` | 61.97 ns | 61.62 ns to 62.45 ns | 1.38 ns | 50 | 32580075 | one pass, output `String` |
| `media_text/html_entity_decode_fragment` | 96.23 ns | 96.02 ns to 97.31 ns | 1.97 ns | 50 | 20910000 | an entity scan plus the output `String` |
| `media_text/hyperlink_escaped` | 264.27 ns | 261.61 ns to 266.08 ns | 8.21 ns | 50 | 7620675 | `escape_html` and `html_attribs`, two allocations |
| `media_text/strip_tags_fragment` | 138.20 ns | 136.99 ns to 141.38 ns | 4.95 ns | 50 | 14129550 | a tag scanner plus the output `String` |
| `media_text/urlencode_url` | 377.84 ns | 374.00 ns to 380.75 ns | 9.45 ns | 50 | 5316750 | one pass, percent-encoding the reserved characters |
| `media_thumbnail/decode_only_jpeg_4000x3000` | 27.169 ms | 26.998 ms to 27.494 ms | 415.787 us | 30 | 90 | the `image` crate's JPEG decoder alone |
| `media_thumbnail/decode_only_png_1000x500` | 3.275 ms | 3.275 ms to 3.276 ms | 7.985 us | 30 | 930 | the `image` crate's PNG decoder alone |
| `media_thumbnail/derivative_bytes_jpeg_4000x3000` | 172.447 ms | 170.600 ms to 173.145 ms | 4.003 ms | 30 | 30 | the three Triangle resizes are 117 ms of it, decode 27 ms, the 800x600 encode 6.7 ms; 21.7 ms stays unattributed |
| `media_thumbnail/derivative_bytes_png_1000x500` | 19.163 ms | 19.065 ms to 19.237 ms | 272.694 us | 30 | 120 | decode is 3.27 ms of it |
| `media_thumbnail/derivative_bytes_png_100x60` | 1.341 ms | 1.338 ms to 1.343 ms | 10.360 us | 30 | 1860 | no enlargement for large and medium, so decode and three encodes dominate |
| `media_thumbnail/derivative_bytes_unsupported_type` | 7.13 ns | 7.11 ns to 7.13 ns | 0.05 ns | 30 | 277692885 | the media-type check, no decode |
| `media_thumbnail/generate_thumbnails_existing` | 1.761 us | 1.685 us to 2.229 us | 657.18 ns | 30 | 1186215 | three `exists` stats, which alone measure about 1.24 us |
| `media_thumbnail/generate_thumbnails_fresh_store` | 19.046 ms | 19.016 ms to 19.100 ms | 270.240 us | 30 | 120 | the transform plus three `put_atomic` writes; it measured 0.1 ms below `derivative_bytes` alone in this session, so the write share is below the drift and is inconclusive |
| `media_thumbnail/generate_thumbnails_unsupported` | 2.129 us | 2.053 us to 2.153 us | 830.39 ns | 30 | 856065 | the three stats, then the type check |
| `media_thumbnail/ingest_seam_thumbnails_png_1000x500` | 19.018 ms | 18.988 ms to 19.058 ms | 102.837 us | 30 | 120 | `derivative_bytes` with the skip swallowed, the call the ingest makes |
| `media_thumbnail/jpeg_encode_only_800x600` | 6.685 ms | 6.676 ms to 6.689 ms | 90.209 us | 30 | 465 | `JpegEncoder::encode_image` on an 800x600 image |
| `media_thumbnail/resize_only_large_to_800x600` | 62.296 ms | 61.179 ms to 63.009 ms | 1.727 ms | 30 | 60 | `resize_exact` with Triangle from 12 Mpx |
| `media_thumbnail/resize_only_medium_to_400x300` | 32.374 ms | 31.477 ms to 32.728 ms | 2.416 ms | 30 | 90 | the same resize at a 10:1 reduction |
| `media_thumbnail/resize_only_square_to_200x200` | 22.242 ms | 22.129 ms to 22.357 ms | 223.713 us | 30 | 90 | the centred 3000x3000 crop resized to 200x200 |
| `media_thumbnail/thumbnail_path_square` | 62.58 ns | 62.50 ns to 62.67 ns | 0.31 ns | 30 | 31741365 | one `format!` |

## What these numbers do and do not cover

**Checklist 1, the limiter.** The limiter is named in the last column of every row. The thumbnail transform is the one
place where the split is measured rather than inferred: `derivative_bytes` on a 4000x3000 JPEG is 172.4 ms, of which the
decoder is 27.2 ms (measured on its own), the three `resize_exact` calls are 116.9 ms, and the 800x600 JPEG encode is
6.7 ms; 21.7 ms (12.6 percent) is not attributed. The two payloads that reach an ingest are also measured on both sides:
`iiif_presentation` is 308.2 us, of which the 99 KB manifest's `serde_json` parse is 184.3 us and its clone into
`MediaData::IiifPresentation` is 79.4 us, 86 percent together. The remaining limiters are read from the source and are
guesses where no second measurement backs them. No profiler was run, so no row here carries profile evidence.

**Checklist 2, was it tuned.** Yes, and uniformly. Criterion's `bench` profile; `criterion` built with
`default-features = false`; 0.5 s warm-up, 2 s measurement, 50 samples (30 for the transform); the 4000x3000 fixture is
encoded once, outside the timed region; the store stands cycle a 64-path window so the write cost is one write and the
tree stays at 64 files. Nothing here compares two implementations. What is not tuned is the machine: the store stands
write to a RAID array that other agents' builds also write to, and the load average moved between 4.2 and 16.1 across the
six sessions. For every row that writes a file I therefore do not report an absolute cost.

**Checklist 4, did it error.** Every bench returns its result through `black_box`. The ingester benches assert per case
that the error store is empty, that the expected files exist under the storage id, and that the fake network was called
the number of times the fixture needs. The thumbnail benches assert that the fixture decodes, that the transform returns
three derivatives, that an unsupported media type is a typed skip, and that the guard store reaches
`ThumbnailOutcome::Generated`. Both crates' full `cargo bench` runs exited 0. What is not counted: no bench counts
failures inside the timed region; nothing benched here can fail by return value at runtime, and a failed fixture is a
panic that aborts the run.

**Checklist 5, does it reproduce.** 50 samples per bench per session (30 for the transform), with criterion's interval
in the table. Across six full sessions: 34 to 39 of the 64 shared benches landed inside plus or minus 5 percent, and 55
to 60 of 64 inside plus or minus 25 percent. The two ends of that behaviour are both worth naming. The transform is
stable: `derivative_bytes/png_1000x500` measured 19.18, 19.02, 18.95, 18.93, 19.39 and 19.16 ms under load averages
from 4.2 to 13.9, a spread of 2.4 percent. The write path is not: `put/1KiB` measured 144.6, 24.7, 79.2, 101.3 and
81.0 us across five of those sessions, and 2.86 ms in the sixth, where the load average reached 14 during the run. The
checked-in session's confidence interval for `put/1KiB` spans 25.9 to 156.0 us. No difference below 25 percent on a
writing row means anything.

**Checklist 7, did it even happen.** The fetch counts and the stored files are asserted per case before the timed run,
so a case that stopped fetching or storing stops the run rather than reporting a fast no-op. The 99 KB presentation
manifest really is parsed and cloned in the timed region: the two decomposition benches measure both halves on the same
document. `cargo bench -p omeka-media -- --test` passed for every bench before the timed runs.

## Numbers I could not explain

- The whole write path. `put/1KiB` (a `create_dir_all` stat, open, write, close) ran 24.7 us to 2.86 ms across six
  sessions, and inside the checked-in session its interval spanned 25.9 to 156.0 us. `put_atomic/1KiB` ran 45.6 us to 5.43 ms.
  The named limiter is the shared array and the shared cores, and the honest reading is that these rows measure the
  machine's queueing, not the store's code. Nothing here should be used to size a write budget.
- `exists_present` measured 412 to 474 ns while `exists_missing` measured 307 to 335 ns, across sessions with tight
  intervals. Each is one `stat`. Why the present case is consistently 100 to 140 ns slower is unexplained.
- `delete/1KiB` costs 3.34 to 3.83 us for one `stat` plus one `unlink`, while `read/1KiB` costs 1.06 us for open, read
  and close. The unlink's extra two microseconds is unexplained, though a directory update journals where a read does
  not.
- `escape_html_fragment` measured 320 ns in one session and 389 ns in another, a 21 percent shift with sub-1 percent
  intervals inside each. Unexplained.
- `generate_thumbnails/fresh_store` measured 0.1 ms *below* `derivative_bytes` alone on the same input in one session,
  although it does strictly more work (three `exists` stats, the transform, three `put_atomic` writes). The write share
  is smaller than the drift between two benches measured minutes apart in the same session, so the write share is
  inconclusive rather than negative.
- 21.7 ms of `derivative_bytes/jpeg_4000x3000` is unattributed as described under checklist 1. It contains the medium
  and square JPEG encodes and the flatten-on-white copy each encoder does, and neither was measured separately.

## Where the per-request work sits

A page render is dominated by the renderers, not by the store. `media_render/file_thumbnail` at 524 ns and
`media_render/iiif` at 4.30 us are the two that build the most markup, and the `iiif` figure is almost entirely a
re-serialise of the stored information document with sorted keys: a plain `to_string` of the same 1 KB document is
0.67 us, so about 3.6 us of the 4.30 us is the `BTreeMap` sort and escape. One ingest of a 99 KB IIIF manifest costs
308 us, of which the JSON work is 86 percent. The file store's own rules cost tens of nanoseconds and are not a limiter.
