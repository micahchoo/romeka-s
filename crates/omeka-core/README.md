# omeka-core

Romeka S is a fork of Omeka S. This crate holds the domain core: typed entities, value rules, the data type
registry, slugs, templates and the permission matrix, ported from upstream PHP. `src/lib.rs` documents the module map
and the upstream file each module mirrors. The crate is pure: no I/O, no async, no database, so everything here is a
pure-function cost.

## Benchmarks

`benches/` holds one criterion target per area. Each is a `harness = false` binary that `criterion_main!` drives.

| target | area | source |
| --- | --- | --- |
| `value_rules` | value and language parsing and validation | `src/value.rs` |
| `datatypes` | the data type registry | `src/datatypes.rs` |
| `site_rules` | slugs, block layouts, block data, templates | `src/site.rs`, `src/template.rs` |

Run one target, all of them, or a filtered set:

```sh
cargo bench -p omeka-core
cargo bench -p omeka-core --bench value_rules
cargo bench -p omeka-core -- value_rules/lang_tag_is_valid
```

Criterion always uses the `bench` profile, which inherits `release`. A debug build makes every number here meaningless.
Nothing in these benches writes to disk, so only the build tree needs `CARGO_TARGET_DIR` pointed off `/tmp`.

Each bench function runs its case once before timing and asserts the expected outcome (an `assert!` block at the top of
each target). If a fixture stopped parsing, or a failure path started succeeding, the run stops instead of quietly
timing the wrong branch. `cargo bench -- --test` runs every bench once and checks those guards.

## Comparing against the baseline

Two ways, for two different jobs.

For a comparison, let criterion do it. It compares distributions and prints a p-value per bench:

```sh
cargo bench -p omeka-core -- --save-baseline before
# change the code
cargo bench -p omeka-core -- --baseline before
```

`benches/baseline.json` is the record of the last full run: for each bench its median, its 95% confidence interval for
the median, its standard deviation, its sample count and its total timed iterations, plus the commit, the rustc version,
the core count and the load average. Regenerate it with:

```sh
./benches/collect_baseline.sh                 # every group
./benches/collect_baseline.sh datatypes       # criterion filters pass through
```

The script clears the previous `new` trees out of the criterion directory first, because criterion keeps every crate's
results in one place and the file must hold this crate's benches only. The file is flat JSON keyed `group/bench`, so
`jq` or `git diff` reads it without tooling.

## The numbers

One session, `bench` profile, 32 cores, rustc 1.93.1. The load average was 3.5 at the start of this run and 2.3 at the
end, on a box six agents share. Each row is a median of 50 samples with criterion's 95% confidence interval for the
median. Iterations is the total number of timed units criterion executed, which for a bench that takes a fresh input
(`convert/*`) is one batch per unit with the setup outside the timed region.

| bench | median | 95% CI | sd | samples | iterations | limiter |
| --- | --- | --- | --- | --- | --- | --- |
| `core_datatypes/convert_literal_from_uri` | 26.43 ns | 26.22 ns to 26.52 ns | 2.03 ns | 50 | 13841400 | a validity check and an enum swap |
| `core_datatypes/convert_resource_noop` | 18.92 ns | 18.89 ns to 18.95 ns | 0.09 ns | 50 | 35501100 | the validity checks only, no swap |
| `core_datatypes/convert_uri_from_literal` | 51.66 ns | 50.75 ns to 52.19 ns | 2.15 ns | 50 | 24256875 | `uri_is_valid` and an enum swap |
| `core_datatypes/fulltext_uri` | 55.31 ns | 55.09 ns to 55.60 ns | 0.73 ns | 50 | 36674100 | `format!`, one allocation and a copy of both parts |
| `core_datatypes/get_for_extract_literal` | 0.41 ns | 0.41 ns to 0.41 ns | 0.00 ns | 50 | 4902390300 | a `match` on the variant returning a `&'static`, no search |
| `core_datatypes/get_for_extract_resource_item` | 0.61 ns | 0.61 ns to 0.62 ns | 0.00 ns | 50 | 3247352325 | a `match` on the variant returning a `&'static`, no search |
| `core_datatypes/get_literal` | 2.96 ns | 2.93 ns to 3.10 ns | 0.10 ns | 50 | 670412850 | a linear scan of six registered names, first entry |
| `core_datatypes/get_missing` | 9.47 ns | 9.46 ns to 9.65 ns | 0.57 ns | 50 | 214522575 | all six compared, then `None` |
| `core_datatypes/get_resource_itemset` | 9.00 ns | 8.65 ns to 9.14 ns | 0.59 ns | 50 | 227373300 | five name comparisons before the hit |
| `core_datatypes/hydrate_invalid_literal` | 14.08 ns | 14.07 ns to 14.09 ns | 0.06 ns | 50 | 141201150 | rejected on validity, no allocation |
| `core_datatypes/hydrate_literal` | 66.22 ns | 66.17 ns to 66.33 ns | 0.26 ns | 50 | 29189850 | lookup, `literal` validity, one `String`, `LangTag::parse` |
| `core_datatypes/hydrate_resource_item` | 15.42 ns | 15.38 ns to 15.48 ns | 0.23 ns | 50 | 126606225 | lookup and an enum build, no allocation |
| `core_datatypes/hydrate_uri` | 163.46 ns | 163.10 ns to 163.98 ns | 1.02 ns | 50 | 12145650 | lookup, two `String` copies, and `uri_is_valid`'s own copy |
| `core_datatypes/is_registered_literal` | 2.63 ns | 2.52 ns to 2.69 ns | 0.11 ns | 50 | 743253600 | `get` plus the `Option` check |
| `core_datatypes/is_registered_missing` | 9.41 ns | 9.41 ns to 9.41 ns | 0.04 ns | 50 | 212055450 | `get` plus the `Option` check, on the miss path |
| `core_datatypes/is_registered_resource_itemset` | 9.02 ns | 8.41 ns to 9.19 ns | 0.54 ns | 50 | 219164850 | `get` plus the `Option` check |
| `core_datatypes/json_ld_literal` | 92.22 ns | 92.10 ns to 92.32 ns | 0.28 ns | 50 | 21717075 | `json!` builds a `Map` of two keys |
| `core_datatypes/json_ld_uri` | 73.01 ns | 72.93 ns to 73.27 ns | 0.72 ns | 50 | 27508125 | `json!` builds a `Map`, fewer keys |
| `core_datatypes/scan_names` | 10.97 ns | 10.96 ns to 10.99 ns | 0.08 ns | 50 | 181239975 | six names collected into one `Vec` |
| `core_datatypes/scan_value_annotating` | 44.99 ns | 44.83 ns to 45.69 ns | 0.74 ns | 50 | 43330875 | six virtual calls plus the `Vec` |
| `core_datatypes/to_string_literal` | 9.55 ns | 9.55 ns to 9.57 ns | 0.16 ns | 50 | 209471025 | one `String` copy of the stored text |
| `core_datatypes/to_string_uri` | 10.48 ns | 10.46 ns to 10.49 ns | 0.07 ns | 50 | 190232550 | one `String` copy of the label |
| `core_site_rules/block_data_parse_browse_preview` | 280.26 ns | 279.80 ns to 280.87 ns | 3.82 ns | 50 | 6923250 | a clone of the object then `from_value` into the struct |
| `core_site_rules/block_data_parse_html` | 13.45 ns | 13.42 ns to 13.49 ns | 0.29 ns | 50 | 142153575 | one `get`, one `as_str`, one `String` |
| `core_site_rules/block_data_to_json_browse_preview` | 242.28 ns | 241.46 ns to 242.96 ns | 5.44 ns | 50 | 8231400 | `to_object`, a `Map` and its `String` keys |
| `core_site_rules/block_layout_from_wire_name_fallback` | 6.40 ns | 6.36 ns to 6.59 ns | 0.14 ns | 50 | 304835925 | the same `match` then one `String` for the unknown name |
| `core_site_rules/block_layout_from_wire_name_known` | 1.44 ns | 1.44 ns to 1.44 ns | 0.01 ns | 50 | 1386789450 | a `match` on the layout names |
| `core_site_rules/block_layout_label` | 5.09 ns | 5.08 ns to 5.10 ns | 0.07 ns | 50 | 394476075 | one `String` per arm |
| `core_site_rules/is_valid_slug_invalid` | 8.84 ns | 8.83 ns to 8.85 ns | 0.03 ns | 50 | 224230425 | `Slug::parse` with the `String` discarded, rejected early |
| `core_site_rules/is_valid_slug_valid` | 12.61 ns | 12.59 ns to 12.66 ns | 0.19 ns | 50 | 157323525 | `Slug::parse` with the `String` discarded |
| `core_site_rules/site_new` | 19.12 ns | 19.10 ns to 19.16 ns | 0.15 ns | 50 | 104168775 | struct build, two `String`s and a `Slug` copy |
| `core_site_rules/site_page_new` | 15.00 ns | 14.97 ns to 15.02 ns | 0.11 ns | 50 | 136844475 | struct build, three `String`s and a `Slug` copy |
| `core_site_rules/slug_parse_empty` | 1.45 ns | 1.45 ns to 1.46 ns | 0.02 ns | 50 | 1372469925 | one length check |
| `core_site_rules/slug_parse_invalid_character` | 7.95 ns | 7.94 ns to 7.97 ns | 0.07 ns | 50 | 248612250 | rejected at the first bad byte, no copy |
| `core_site_rules/slug_parse_long` | 91.53 ns | 90.47 ns to 92.21 ns | 3.81 ns | 50 | 21752775 | the same scan over 200 bytes |
| `core_site_rules/slug_parse_valid` | 11.42 ns | 11.37 ns to 11.49 ns | 0.14 ns | 50 | 177416250 | one byte scan then one `String` |
| `core_site_rules/slugify_long_body` | 172.10 ns | 171.60 ns to 173.18 ns | 3.15 ns | 50 | 11413800 | the same pass over about 230 chars (0.8 ns per char) |
| `core_site_rules/slugify_non_ascii` | 22.02 ns | 22.00 ns to 22.03 ns | 0.05 ns | 50 | 90763425 | multi-byte chars each become one dash; the pass is over chars |
| `core_site_rules/slugify_short_title` | 23.67 ns | 23.62 ns to 23.72 ns | 0.17 ns | 50 | 83455125 | one char pass, output `String` with capacity reserved |
| `core_site_rules/template_missing_required_props` | 9.17 ns | 9.15 ns to 9.18 ns | 0.07 ns | 50 | 214467750 | 10 props tested against 3 present ids |
| `core_site_rules/template_normalize_data_types_with_duplicates` | 40.27 ns | 40.21 ns to 40.37 ns | 0.29 ns | 50 | 49587300 | trim then a linear `Vec::contains` per entry |
| `core_site_rules/template_unknown_data_types` | 89.36 ns | 89.07 ns to 92.54 ns | 3.74 ns | 50 | 22538175 | 10 props, 2 names each, against the six-entry registry scan |
| `core_value_rules/lang_tag_is_valid_en` | 17.33 ns | 17.30 ns to 17.34 ns | 0.17 ns | 50 | 117083250 | one `split('-')` plus the `Vec<&str>`, one subtag |
| `core_value_rules/lang_tag_is_valid_en-US` | 28.84 ns | 28.78 ns to 28.92 ns | 0.20 ns | 50 | 67917975 | two subtags |
| `core_value_rules/lang_tag_is_valid_en-US-u-ca-gregory` | 83.29 ns | 83.15 ns to 83.48 ns | 0.94 ns | 50 | 23926650 | five subtags, and `parts.remove(0)` shifts the tail so the walk is quadratic in subtags (inferred from `value.rs`, the scaling is measured) |
| `core_value_rules/lang_tag_is_valid_english-invalid` | 18.07 ns | 18.04 ns to 18.13 ns | 0.23 ns | 50 | 110607525 | rejected by the length rule before the `Vec` is built, so it costs one subtag |
| `core_value_rules/lang_tag_is_valid_sr-Latn-RS` | 38.25 ns | 38.22 ns to 38.30 ns | 0.71 ns | 50 | 51776475 | three subtags |
| `core_value_rules/lang_tag_parse_invalid` | 23.06 ns | 23.02 ns to 23.12 ns | 0.27 ns | 50 | 86257575 | the reject path copies nothing |
| `core_value_rules/lang_tag_parse_valid` | 90.08 ns | 89.94 ns to 90.26 ns | 0.77 ns | 50 | 22020525 | validation plus one `String` copy |
| `core_value_rules/literal_is_valid_clean` | 2.27 ns | 2.27 ns to 2.29 ns | 0.03 ns | 50 | 875645775 | `trim()`, no allocation |
| `core_value_rules/literal_is_valid_whitespace_only` | 2.66 ns | 2.63 ns to 2.67 ns | 0.06 ns | 50 | 743624625 | `trim()`, no allocation |
| `core_value_rules/uri_is_valid_clean` | 112.66 ns | 111.76 ns to 113.81 ns | 2.04 ns | 50 | 17285175 | one fresh `String` from `filter`+`collect` then a char scan (46-byte input) |
| `core_value_rules/uri_is_valid_control` | 65.91 ns | 65.29 ns to 66.55 ns | 1.80 ns | 50 | 26180850 | the same copy, rejected by the control-char scan |
| `core_value_rules/uri_is_valid_javascript` | 73.28 ns | 72.87 ns to 73.62 ns | 1.40 ns | 50 | 23560725 | the same copy, rejected after the scheme scan |
| `core_value_rules/uri_is_valid_whitespace` | 75.37 ns | 75.05 ns to 75.85 ns | 1.80 ns | 50 | 26379750 | the same copy, input with tabs and newlines |
| `core_value_rules/value_data_literal` | 8.98 ns | 8.97 ns to 8.99 ns | 0.05 ns | 50 | 219510375 | `literal_is_valid` plus one `String` from the input |
| `core_value_rules/value_data_uri` | 79.98 ns | 79.52 ns to 80.50 ns | 1.41 ns | 50 | 22414500 | `uri_is_valid` on a 22-byte URI plus one `String` |
| `core_value_rules/value_object_parse_list_of_25` | 4.827 us | 4.815 us to 4.847 us | 37.00 ns | 50 | 423300 | 25 objects at the single-object cost plus the `Vec` |
| `core_value_rules/value_object_parse_literal` | 145.97 ns | 145.27 ns to 147.53 ns | 2.49 ns | 50 | 13163100 | `serde_json::from_value` clones the input `Value` and maps it key by key |
| `core_value_rules/value_object_parse_resource` | 114.02 ns | 113.72 ns to 114.17 ns | 0.82 ns | 50 | 17306850 | the same clone and map, fewer keys |
| `core_value_rules/value_object_parse_uri` | 149.20 ns | 148.88 ns to 149.41 ns | 1.57 ns | 50 | 13105725 | the same clone and map |
| `core_value_rules/value_to_json_literal` | 230.72 ns | 229.34 ns to 232.05 ns | 3.03 ns | 50 | 9292200 | `to_object` then `serde_json::to_value`, which builds a `Map` of six keys |
| `core_value_rules/value_to_object_literal` | 31.33 ns | 31.19 ns to 31.55 ns | 0.60 ns | 50 | 58239450 | three small `String` copies into the wire struct |
| `core_value_rules/value_to_object_resource` | 21.96 ns | 21.94 ns to 21.99 ns | 0.09 ns | 50 | 85662150 | two enum fields, no text to copy |
| `core_value_rules/value_to_object_uri` | 34.09 ns | 33.87 ns to 34.23 ns | 0.35 ns | 50 | 49127025 | as the literal plus the label copy |

## What these numbers do and do not cover

**Checklist 1, the limiter.** The limiter is named in the last column of every row. Two mechanisms carry most of this
crate: a `String` copy at each parse and each output call, and the `Vec<&str>` that `lang_tag_is_valid` builds. Rows
scale as those mechanisms predict, which is the evidence behind the labels: `value_data_uri` costs 80 ns on a 22-byte
URI while `uri_is_valid` alone costs 113 ns on a 46-byte one, so the fresh `String` dominates and not the char scan;
`lang_tag_is_valid` costs 17, 29, 38
and 83 ns for one, two, three and five subtags; `slugify` costs 23.7 ns for 22 chars and 172 ns for 230, one pass at
0.75 ns per char. Where no scaling row exists the limiter is read from the source and is a guess, not a measurement. No
profiler was run on this crate, so no row here carries profile evidence.

**Checklist 2, was it tuned.** Yes, and uniformly. Criterion's `bench` profile (opt-level 3, no debug assertions);
`criterion` built with `default-features = false`, so no charting or thread pool is linked in; 0.5 s warm-up, 2 s
measurement, 50 samples (30 for the media transform); fixtures built once, outside the timed region; `iter_batched`
where a case needs fresh input. Nothing here compares two implementations, so there is no untuned side. The machine is
not tuned: six agents share 32 cores, and the load average moved between 2.3 and 16 across the sessions.

**Checklist 4, did it error.** Every bench returns its result through `black_box`, and criterion panics on anything a
fixture does not produce. The guard block in each target asserts the shape of each case before timing (each parse Ok or
Err as benched, the hydrate error path errors, the block data parses, the required-property check finds its one missing
property). Both crates' full `cargo bench` runs exited 0. What is not counted: no bench counts failures inside the timed
region, because nothing benched here can fail by return value at runtime; a failed fixture is a panic that aborts the
run.

**Checklist 5, does it reproduce.** 50 samples per bench per session, with criterion's confidence interval in the
table. Across five full sessions on this machine, of which four are recorded for comparison, the median
session-to-session ratio ran 0.96 to 1.00, with 31 to 53 of the 65 benches inside plus or minus 5 percent depending on
which pair of sessions is compared. Three rows fall outside
plus or minus 20 percent between sessions (`to_string_literal`, `to_string_uri`, `scan_names`, all 10 to 20 ns and
moving by up to 2x while their in-session interval stays under 1 ns wide). Read that as the resolution limit: a row
above about 100 ns reproduces within a few percent, and for the sub-30 ns rows and the file-write rows on `omeka-media`
a difference smaller than 20 percent means nothing.

**Checklist 7, did it even happen.** The fixtures are real inputs, the results are returned through `black_box`, and
criterion would report a panic rather than a time if a case stopped working. `cargo bench -p omeka-core -p omeka-media
-- --test` passed for every bench in both crates before the timed runs.

## Numbers I could not explain

- `to_string_literal` measured 20.1 ns in one session and 10.2 ns in another, `to_string_uri` 21.2 against 11.2 ns, and
  `scan_names` 19.6 against 12.0 ns, each with a confidence interval under 1 ns wide inside its session. All three
  allocate one small `String` or `Vec`. The allocator's fast path is the obvious suspect and I did not measure it, so
  the 2x shift across sessions is unexplained.
- `uri_is_valid_javascript` measured 101 ns and then 74 ns, with a 1 to 2 ns interval inside each session, while the
  same function on other inputs stayed within 5 percent. Unexplained.
- `hydrate_literal` (69.3 then 65.2 ns) and `fulltext_uri` (73.0 then 60.0 ns) moved in the same direction as the load
  average. Session noise is the guess, not a measurement.
- `is_registered_missing` (9.2 ns) sits 3 percent from `get_missing` (9.6 ns) although `is_registered` is `get` plus an
  `Option` check. That gap is below what this harness resolves.

## Share of the whole

The value parse is not a limiter for a request. `value_object_parse/list_of_25` is 4.83 us for a 25-value list body,
against the 19.8 ms `/api/items` median recorded in `briefs/perfharness.md`. That is about 0.02 percent of the request,
and the 19.8 ms figure is a debug-build ballpark with one run behind it, not a measurement of mine, so read the ratio as
an order of magnitude only. The media renderers and the thumbnailer are where the per-request work in this port sits.
