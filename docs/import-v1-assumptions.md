# Import v1: the 88 assumptions

The affordance read of 2026-10-20 found these assumptions in the v1 import. Import v2 closes each row: a test names its id, or docs/compat.md records it as an accepted limit. Severity says what a person saw when an archive broke the assumption.

## File names → items

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A01 | Name parts use ASCII letters, so an example in another script does not match its own path | `example.ts:65` | पत्र_001_आगे.jpg, José_001.jpg, Kraków_001.jpg, LT_௧௨.jpg | blocked | ran | |
| A02 | A run of digits keeps the example's width | `example.ts:65` | L_7, L_12, L_130; LT_999 vs LT_1000; 1950-03-12 vs 1950-3-12 | visible | ran | |
| A03 | Delimiters are only _ - . and space | `example.ts:33` | BHC006(1).jpg, Smith, John 1901.jpg, full-width ＿, a danda | blocked | ran | |
| A04 | Parts split only at a delimiter, never at a change of case or digit | `example.ts:46` | BHC006front.jpg / BHC006back.jpg become two items | silent | ran | |
| A05 | Each part keeps one letter case | `example.ts:69` | _Front, _FRONT, lt_001_front | visible | ran | |
| A06 | Every name has the same number of delimiters and words | `example.ts:96` | 1924-01-02 Mary Smith to Jones.pdf; O’Brien; double spaces | visible | ran | |
| A07 | Every file has an extension, or none does, like the example | `example.ts:40` | extension-less files; scan.001.tar.gz gives identifier scan.001.tar | visible | ran | `crates/omeka-api/src/import2/derive/tests/steps.rs` `a07_the_default_pattern_takes_one_extension_or_none` |
| A08 | By default the whole name is the identifier | `example.ts:203` | the demo: front and back become separate items until “back” is unmarked | silent | ran | |
| A09 | A variant suffix can be optional only when marked part or page | `example.ts:221` | “back” left as Nothing gives no hint; PH_101/102/201/202 missing | visible | ran | |
| A10 | One identifier group: a second mark becomes identifier2, and the server ignores it | `pattern.rs:15` | Box 4/Folder 12 and Box 4/Folder 13 merge into one item | silent | ran | |
| A11 | The example is the first file in byte order, of any type | `folder.tsx:177` | a README.txt at the top gives a pattern that matches 1 file | visible | ran | |
| A12 | Only the top folder can be marked as the set | `example.ts:57` | a flat folder where the series is AF or PH in the name | blocked | read | |
| A13 | An identifier is unique across the whole folder, not per subfolder | `grouping.rs:304` | Letters/001.jpg and Photos/001.jpg merge into one item in two sets | silent | read | |
| A14 | Identifiers match exactly: case and Unicode form | `grouping.rs:309` | BHC006_front vs bhc006_back; macOS NFD Kraków vs NFC | silent | ran | |
| A15 | More than 5 files per item means the identifier is too short | `files.tsx:22` | a 200-page diary raises a false alarm | misleading | read | |
| A16 | Media order is byte order; page is never read; no primary media is set | `archive.rs:141 · run.rs:424` | p10 before p2, back before front; the first file drives the “first” tag rule | silent | ran | |
| A17 | Every file is a candidate item, whatever its type | `inputs.rs:43` | _scratch/notes.txt, Thumbs.db, desktop.ini, other CSVs become items in a set that starts on | silent | ran | |
| A18 | Names are UTF-8, folders are not dot folders or symlinks | `archive.rs:185` | Latin-1 names from a Windows ZIP, .originals/, a tree of year symlinks: dropped from every count | silent | read | |
| A19 | The import folder never moves | `imports.rs:217` | a renamed folder: every preview errors, and the folder cannot be pointed elsewhere | blocked | read | |
| A20 | The archive is small enough to walk on every preview | `inputs.rs:26 · archive.rs:119` | a root of 500,000 files | visible | read | |

## Folders → item sets

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A21 | Sets come from folders or a hand regex; no CSV column names a set | `grouping.rs:73` | a flat folder: no sets, so no type can be chosen | blocked | ran | |
| A22 | A second-level folder name is unique across parents | `grouping.rs:79` | Letters/1950s + Photographs/1950s become one set “1950s” | silent | ran | |
| A23 | Level 2 exists everywhere | `grouping.rs:79` | Artifacts/ beside Photographs/1950s/ gives a mixed list | visible | ran | |
| A24 | “Same name” means the same case | `catalog.rs:91` | an existing “Photographs” and a folder “photographs”: a second set | silent | ran | |
| A25 | A set name is not blank | `work.rs:218` | a cleared name makes a set titled “” | silent | ran | |
| A26 | Two rows with one name want one set; Check and Run differ on a trailing space | `check.rs:87` | “Obj” and “Obj ” show as two on Check, one in the run | silent | ran | |
| A27 | Existing set titles are unique | `catalog.rs:208` | two sets titled “Letters”: the lowest id wins | silent | read | |
| A28 | Every item belongs to a set that is on | `grouping.rs:162` | items wanted without a set are dropped; the badge is wrong under “both” | misleading | ran | |

## Sets → item types

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A29 | The type comes from the item's first set | `derive.rs:377` | under “both” the 1950s type does nothing; an identifier with files in two folders | silent | ran | |
| A30 | A staged new type is identified by its name | `work.rs:249` | two “Thing” types, from Person and Letter: one template with Person's fields | silent | ran | |
| A31 | The admin's starter fields are the server's | `starters.ts:50 · catalog.rs:199` | any reader of “It starts with these fields”: Letter lists created/spatial/description, the server makes identifier/date | silent | ran | |
| A32 | Six family-archive genres cover the archive | `catalog.rs:199` | artifacts, maps, audio, newspapers: only “Other” | visible | read | |
| A33 | Base Resource exists with that English label | `catalog.rs:26` | a renamed or deleted Base Resource: items get no template | silent | ran | |
| A34 | A required title can be filled | `derive.rs:495` | folder-only archive with a starter type: every row left out, the type never made | visible | ran | |
| A35 | bibo and foaf are installed | `catalog.rs:237` | an install without BIBO: termUnknown, and no fix but another starter | blocked | ran | |
| A36 | A new type's name is free | `derive.rs:301` | a second import accepts the prefilled “Photograph” | visible | ran | |
| A37 | A created template needs no class or description, and the grid can add fields | `run.rs:276 · work.rs:251` | “You can add more from the grid” adds values, not template fields | misleading | read | |

## The spreadsheet

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A38 | One CSV per import | `plan.rs:28` | objects.csv + photos.csv; a register split by year (the other CSVs become import files) | blocked | ran | |
| A39 | Only .csv files are sheets | `inputs.rs:43` | register.tsv, .txt, .xlsx, .ods: “This folder has no CSV file” | blocked | ran | |
| A40 | Encodings are UTF-8 or Windows-1252 | `csv/sheet.rs:99` | UTF-16 headers with hidden NULs; Shift-JIS read as 1252; a mostly-UTF-8 file with 1252 on spoils मुंबई | silent | ran | |
| A41 | Delimiters are comma, semicolon, tab | `plan.rs:122` | a pipe-delimited export: error with no option | blocked | ran | |
| A42 | Lines end in LF or CRLF | `csv/reader.rs:79` | CR-only “CSV (Macintosh)”: one record, 0 rows, no error | silent | ran | |
| A43 | Quoting is RFC 4180 | `csv/reader.rs:63` | backslash escapes, single quotes, text after a quote: “check the header row” | misleading | ran | |
| A44 | Data rows are no wider than the header | `csv/sheet.rs:176` | a trailing delimiter on data rows only: headerRow error | misleading | ran | |
| A45 | The header is in lines 1–5 or where detection put it | `sources.tsx:30` | six preface lines and a wrong detection | blocked | read | |
| A46 | Headers are unique and not empty | `plan.rs:118` | two “Subject” columns: the second shows the first's values | silent | read | |
| A47 | The join is on the identifier, exact and case-sensitive | `derive.rs:342` | x_003 vs X_003; 007 vs 7 after Excel; B 4 vs B_4: two half-items | visible | ran | |
| A48 | One row per item; the first row wins, and duplicates are not counted | `derive.rs:343` | a register with one line per copy or accession event | silent | ran | |
| A49 | Rows describe items, not files; CSV values never reach media | `values.rs:129` | a register with one row per image | blocked | read | |
| A50 | One value per cell; two columns on one term keep the first | `values.rs:149` | Creator 1 / Creator 2; cats\|dogs; Smith; Jones | silent | ran | |
| A51 | Values have no language | `rows/values.rs:84` | Title_en and Title_hi; “@en Hello” | silent | ran | |
| A52 | Values are literals, never URIs or links | `run/lookups.rs:36` | a Rights URI column; “Part of: BHC_006_AF_001” | silent | read | |
| A53 | A new field's label maps to one term | `plan.rs:172` | Shelf mark, Shelf-Mark, SHELF MARK all become local:shelfMark | silent | ran | |
| A54 | A column mapped to dcterms:identifier is used | `derive.rs:442` | an old-id column vanishes, and is not listed as unmapped | silent | ran | |
| A55 | A short or long line keeps values by position | `derive.rs:523` | an unquoted comma shifts date into creator; “Keep the values” accepts it | silent | ran | |
| A56 | The repair counts describe the result | `csv/sheet.rs:231` | the space count includes the title line; the 1252 count does not fall after repair | misleading | ran | |

## File tags

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A57 | Tags are EXIF, a fixed list of 11 | `tags.rs:31` | IPTC captions, XMP keywords from Lightroom, PDF info, ID3: invisible, refused if named | blocked | ran | |
| A58 | EXIF text is UTF-8 | `image_metadata.rs:217` | a 1252 Artist from an old tool: U+FFFD, no repair | visible | read | |
| A59 | GPS to two decimals as text is enough | `image_metadata.rs:245` | about 1 km of precision, written as a literal | silent | read | |
| A60 | The extension tells the format | `tags.rs:164` | a misnamed file gives a wrong Format value | silent | read | |
| A61 | An edit of a multi-valued tag field keeps the values | `values.rs:20` | “Rao ; Iyer” written as one literal | silent | ran | |

## Values and problems

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A62 | A date is YYYY, YYYY-MM or YYYY-MM-DD | `problems.rs:74` | c. 1890, 1890s, 1890~, 189X, 1890/1895, 31/12/1890, n.d.: row left out, and no bulk fix | visible | ran | |
| A63 | Only the nine dcterms date terms are dates | `problems.rs:55` | a date in local:dateOfBirth passes as any text | silent | read | |
| A64 | Empty matters only for required fields | `derive.rs:495` | Base Resource has none, so blank titles import silently | silent | read | |
| A65 | Rows with problems should be left out; there is no import-anyway | `check.rs:67 · check.rs:105` | a large archive whose keeper accepts imperfect dates | visible | ran | |
| A66 | Encoding damage is U+FFFD or 1252 mojibake | `csv/windows1252.rs:60` | MacRoman, double-encoded UTF-8 | silent | read | |
| A67 | Sheet repairs fix every space and encoding problem | `grid-fixes.ts:38` | “Remove the spaces” is offered for tag and edit values it never touches | misleading | ran | |
| A68 | Edits stay valid when the pattern changes | `plan.rs:32` | edits keyed by old identifiers apply to nothing | silent | read | |
| A69 | One priority for the whole import: sheet or files | `values.rs:109` | EXIF Artist right for photos, wrong for scans | silent | ran | |

## Install, run and undo

| Id | Assumption | Where (v1) | An archive that breaks it | Seen as | Evidence | Closed by |
| --- | --- | --- | --- | --- | --- | --- |
| A70 | One archive root per server; symlinks are never followed | `config.rs:80` | collections on two disks or NAS shares | blocked | read | |
| A71 | The files are already on the server | `compose.archive.yaml:16` | a volunteer with files on a laptop; a hosted install without a shell | blocked | read | |
| A72 | Create only; an identifier is unique across the whole install | `catalog.rs:161` | a second collection also numbered 001 is skipped as “exists”; a new back scan is never attached | silent | ran | |
| A73 | Running again with nothing new is an error | `imports.rs:385` | re-running to check that everything is in: 409 | visible | ran | |
| A74 | Undo is one level and global | `import_lifecycle.rs:309` | undo import 2, then import 1 stays permanent, and the 409 says it is not the last | misleading | ran | |
| A75 | Any run start may close the previous window | `import_lifecycle.rs:309` | a run that writes 0 items ends “Done.” and takes the window | silent | ran | |
| A76 | One import or undo at a time, install-wide | `import_lifecycle.rs:295` | several archivists on one install | visible | read | |
| A77 | A row that fails does not fail the run | `run.rs:353` | “Done.”, with raw English OS errors only in left.csv and the log | misleading | ran | |
| A78 | Everything created is public, joins synced sites, and new sets are closed | `rows/resources.rs:73 · run.rs:253` | embargoed or restricted material | silent | ran | |
| A79 | The owner is whoever made the draft | `import_archive.rs:144` | admin A drafts, admin B runs | silent | read | |
| A80 | The media type is a raster image or UTF-8 text | `media_type.rs:21` | PDF, MP4, WAV become application/octet-stream and get no player | silent | ran | |
| A81 | Thumbnails for gif, jpeg, png, tiff, webp | `thumbnailer.rs:32` | BMP, PDF, video, HEIC, JPEG 2000 | visible | ran | |
| A82 | No file-type allow list is needed | `ingest/mod.rs:317` | Thumbs.db became the item “Thumbs” | silent | ran | |
| A83 | A whole file fits in memory | `media_files.rs:389` | multi-GB video in a memory-limited container | visible | read | |
| A84 | Media are stored on a local FileStore | `run.rs:134` | object storage | blocked | read | |
| A85 | The folder stays as it was after the run | `left.rs:9` | left.csv is a 404 after a move, or describes today's files | misleading | ran | |
| A86 | The database is SQLite (a MySQL migration exists, but run and undo take &SqlitePool) | `run.rs:24 · undo.rs:21` | a MySQL install | blocked | read | |
| A87 | Run and left.csv derive within one request (30 s timeout) | `imports.rs:384` | 50,000 files with EXIF rules and a cold tag cache | visible | read | |
| A88 | The operator guide's recovery steps still apply | `operator-guide.md:103` | “run the job by hand” is refused for a dead import | misleading | read | |
