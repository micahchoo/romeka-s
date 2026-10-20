# Import v2: contract

This file is the contract every import v2 unit builds against. A unit that needs a change reports it
to the coordinator; it does not drift. The v1 contract and run log stay in `docs/archive-import.md`.
The v1 findings, with stable ids A01–A88, are in `docs/import-v1-assumptions.md`.

Why v2 exists: an affordance read of v1 (2026-10-20) found 88 assumptions about the archive's shape,
41 of them silent. v1 read one fixed signal for each structural decision (item from the file name,
set from the folder, type from the set, join on the exact identifier) and let a person correct
values but never structure. v2 keeps the rule as the primary, reviewable artifact and adds
decisions: corrections that survive the next derive. The goal is upstream CSV Import parity
(CSVImport 2.6.2) plus the extras under "Beyond upstream".

## Definition of done

1. Every unit in the Units table is merged into main with a verdict in the run log:
   `unit-test-verified` or better, `live-ui-verified` for admin units.
2. The scenario suite passes as e2e specs on SQLite; the server-side scenarios also pass on MySQL 8
   and MariaDB 11.
3. Every row A01–A88 has its "Closed by" cell filled: a test name, or a `docs/compat.md` row that
   records an accepted limit and its reason.
4. Gates pass: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, admin `typecheck`, `lint`, `test`, `build`, `e2e`, `perf:check`. New
   admin strings exist in every locale, and changed English strings are translated again.

## Words

Each term has one meaning, in code and in prose.

- **Import**: one stored record (`romeka_import`). A *draft* until it runs, then a *ledger entry*.
- **Source**: where evidence comes from: a **folder** under a named archive root, or the import's
  **staging** area (uploads). A source has a stable `id` in the plan.
- **Evidence index**: the list of entries the import last scanned, with size, modified time and
  sniffed type. It is cached per import and rebuilt by a **rescan** (on request, and at run start).
- **Entry**: one unit of evidence with a fixed identity: a **file** (`{ source, path }`, the path
  relative to the source, NFC) or a **row** (`{ sheet, key }`, see "Row keys").
- **Sheet**: a CSV, TSV or ODS file in a source, read as rows. A sheet is never a file entry.
- **Group key**: the text of the pattern's `identifier` group, NFC, optionally case-folded. Files
  with one group key form one *file group*.
- **Record**: one resource the import proposes to write: an **item**, a **media**, or an **item
  set**. Each file in a kept file group is a media record under that group's item record.
- **Slot**: one property of a record: `sets`, `template`, `class`, `owner`, `visibility`, `sites`,
  `mediaOrder`, `ingester`, `mediaSource`, `itemKey`, or a property term (`dcterms:title`).
- **Source ref**: where a slot's value comes from: a pattern group, a folder level, a sheet column,
  a file tag, a file fact, or a constant.
- **Rule**: the pattern, the sheet plans and the slot rules.
- **Decision**: a correction keyed by an entry. Decisions apply after the rule, in list order; a
  later decision on the same target wins.
- **Orphan**: a decision whose entry is not in the evidence index, or no longer lands in a record of
  the kind it targets. Orphans are listed and not applied. They are never dropped.
- **Flag**: a finding about a value, an entry, a record or the plan, with a *kind* and a *policy*.
- **Action**: what records do to the install: `create`, `append`, `revise`, `update`, `replace`,
  `delete`.
- **Match**: finding the existing resource a record acts on, by a key slot within a scope.
- **Touched**: an imported resource changed by anything but its import after the import wrote it.
- **Before-image**: the stored state of a resource as it was before an import changed it.
- **Digest**: a hash of the derived model that Check shows and Run must see again.

## Evidence

- **Walk.** Each folder source is walked from its path. Symlinked folders are not followed, dot
  files and dot folders are not entries, and file names that are not UTF-8 are decoded as cp437
  (ZIP origin) or Latin-1 and flagged `name-encoding`. Nothing is dropped silently: the counts of
  symlinks, dot entries and decoded names are anomalies.
- **Index.** The scan writes the evidence index. Previews read the index, never the disk, so a
  preview costs the same on a 500,000-file root once scanned. A rescan runs as a job when the
  source holds more than 20,000 entries, else inside the request.
- **Media type.** Each file's type is sniffed from its bytes (first 8 KB) by `omeka-media`. A file
  whose type or extension the install's file-validation settings refuse is flagged `not-media`.
  `format-mismatch` marks a file whose extension disagrees with its sniffed type.
- **Bytes.** No reader loads a whole file over 64 MB into memory. EXIF reads seek; the store takes
  streams.

## Sheets

- **Read options**: encoding (`auto` reports its guess and a confidence), delimiter (`auto`, `,`,
  `;`, tab, `|`, `:`, space), quote (`"`, `'`, none), header line (any line), trim. CR, LF and CRLF
  line ends all parse. A quote that does not close, or text after a closing quote, flags
  `cells-count` on that row with the raw line; the reader never shifts later rows.
- **Encodings decoded**: UTF-8 (BOM dropped), UTF-16 LE/BE (BOM or detected), Windows-1252,
  ISO-8859-1. A detected encoding with no decoder (Shift-JIS, ISCII, MacRoman) gives the blocking
  sheet flag `encoding-unsupported` that names it.
- **Columns** are keyed by position and header: `{ index, header }`. A duplicate or empty header is
  flagged on the sheet and stays mappable by index. Headers starting with `#` are never mapped.
- **Short and long rows**: missing cells are empty; extra cells are kept as unmapped values. Both
  flag `cells-count`, default policy `leave`.
- **Counts** on the Sources tab are computed after the repairs the plan turns on, and exclude the
  lines above the header.
- **Row keys.** A row's key is the value of the sheet's key column (its join column, else its item
  key column), NFC and trimmed, plus `#n` for the n-th row with that value. A sheet with no key
  column keys rows by line. When a sheet's bytes change (the plan stores each sheet's hash),
  decisions keyed by line become orphans; decisions keyed by value keep their rows.

## The derive

`derive(index, sheets, rule, decisions, catalog) -> Model` is pure. The order is fixed:

1. **Group files** by group key. A file the pattern does not match is an *unmatched entry*. Only
   files with a media type are candidates for the example the admin offers.
2. **Apply `move` decisions.** A file joins the group of another file, starts a group of its own
   (its own path is the group key), or leaves.
3. **Join sheets.** An item row joins a file group when its join cell equals the group key (or, for
   a `fileName` join, when the cell names one of the group's files), after the join's
   normalisation: NFC always; optional case fold, leading-zero strip, and separator fold (`_`, `-`,
   `.`, space treated as one). A media row attaches to the item record whose `itemKey` equals its
   item key cell. An item-set row is an item-set record. A second row with the same join key flags
   `duplicate-row` (default `leave`); only the first joins.
4. **Make records.** Each kept file group is an item record with one media record per file. Each
   item row with no file group is an item record with no media (`no-file`). Each media row is a
   media record (a file path in a source, or an ingester and media source).
5. **Resolve slots.** A source ref yields a list of values. `combine: first` takes the first
   non-empty list; `combine: all` takes the union. Values are distinct by `(type, text, lang)`. A
   column ref on a record with no joined row yields nothing. On a single-valued slot (`template`,
   `class`, `owner`, `visibility`, `mediaOrder`, `ingester`, `mediaSource`, `itemKey`) only the first
   value counts and more values flag `many-values`. Column options (split, type, language, private,
   trim) shape values before they are counted. Then `set` decisions replace the slot's values.
6. **Match.** Each record kind has its own key and scope (`action.match`). Zero matches on a
   non-create action: `unmatched` (policy `action.unmatched`). Several: `many-matches`; a `match`
   decision chooses. On `create`, a key that exists in scope flags `exists` (default `leave`).
7. **Flag, report, digest.** Value flags, record flags, anomalies, orphans, and the digest.

All comparisons (group keys, joins, titles, labels, match keys) happen in Rust on loaded values,
never through SQL collation, so SQLite and MySQL answer the same.

### Slot rules a worker must not guess

- **Empty.** A value that is empty after trim is no value. It never creates a set, type or field.
- **Value map.** A slot rule's `map` replaces a value whose text (compared as titles) is a key with
  the mapped text, before the value is used: `AF` → `Artifact` names a template, `Photographs` →
  `Photograph`. A key mapped to `""` drops the value, which is how an item set is turned off.
- **`dcterms:identifier`** of an item record defaults to the group key (or the join cell for an item
  with no files) unless the plan gives the term its own slot rule. Its first value is the item's
  `itemKey` unless `itemKey` has a rule.
- **Sets.** A value names an item set by id, by this import's item-set record (its `itemKey`), or by
  title. Titles compare after trim, NFC and case fold. One match: used. Several: `many-matches`.
  None: a new set is created, open (`is_open = true`), with the record's visibility. A record whose
  every set is off is still a record; turning a set off removes only the membership.
- **Template.** A value names a template by id or label (compared as titles). None: a `newTypes`
  entry with that label; else `unknown-type`, and the record gets `defaults.template`, else the
  install's "Base Resource", else none, which Check states.
- **Owner**: a user id or an email. A global admin may name any user; a site admin only themself.
  `runner` is the user who presses Run (on the CLI, the user given by `--as`).
- **Visibility**: `public` or `private`; `false`, `no`, `off`, `0` and `private` mean private.
- **Media order.** Media sort by `mediaOrder` when set, else by path, with a natural sort (`p2`
  before `p10`). The first media is the item's primary media.
- **Row kinds** in a kind column: `item`, `items`, `media`, `item set`, `itemset`, `item sets`,
  `itemsets`, case-insensitive. Other values flag `unknown-kind` (default `leave`).
- **New types from the mapping** get one field per term that a record using the type writes;
  `dcterms:title` is required (`foaf:name` when the class is `foaf:Person`).
- **New fields.** A slot rule may name a term in `newTerms`; the run creates it in the `local`
  vocabulary. Its term is `local:` plus the lower camel case of its label (letters and digits of any
  script). Two labels that give one term are refused at the boundary.
- **Dates.** `date-form` applies to values of the nine Dublin Core date terms and of template
  fields whose data type is a date. A value that is not EDTF level 0–1 is kept as written and
  flagged; an EDTF value is stored as written.
- **GPS** values are decimal degrees to 6 places, `lat, lon`, a literal.

### Decisions

```ts
type EntryRef = { file: { source: string; path: string } } | { row: { sheet: string; key: string } };
type Decision =
  | { kind: 'move'; file: EntryRef; to: { with: EntryRef } | 'own' | 'leave' }
  | { kind: 'leave'; entry: EntryRef }                       // leave the record that holds it out
  | { kind: 'set'; entry: EntryRef; target: 'item' | 'media' | 'itemSet'; slot: SlotName;
      values: DecisionValue[] }                              // [] clears the slot
  | { kind: 'match'; entry: EntryRef; target: 'item' | 'media' | 'itemSet'; resource: number | null }
  | { kind: 'flag'; entry: EntryRef; flag: FlagKind; slot: SlotName | null; policy: FlagPolicy }
  | { kind: 'pair'; row: { sheet: string; key: string }; file: { source: string; path: string } };
interface DecisionValue { type: ValueType; text: string; lang: string | null; label: string | null }
```

A decision targets the record of `target` kind that holds the entry: for a file, its media record
(`media`) or its group's item record (`item`). A row entry targets the record the row made. When two
decisions on different entries of one record set the same slot, the later one in the list wins and
the anomaly `decision-overlap` lists them.

## The plan (stored JSON, sent by the admin)

camelCase, `version: 2`, parsed at the boundary, unknown members refused. A v1 plan is not parsed:
v1 imports stay readable as history, and `copyFrom` a v1 import answers 409 `v1Plan`.

```ts
interface ImportPlanV2 {
  version: 2;
  sources: SourcePlan[];
  grouping: { pattern: string | null; caseFold: boolean };
  example: Example | null;                // admin state, never read by the server
  sheets: SheetPlan[];
  slots: { item: SlotRules; media: SlotRules; itemSet: SlotRules };
  action: ActionPlan;
  defaults: { visibility: 'private' | 'public'; owner: 'runner' | number; sites: number[];
              template: number | null };
  flags: Partial<Record<FlagKind, FlagPolicy>>;   // a missing kind uses its default
  decisions: Decision[];
  newTypes: NewType[];
  newTerms: Array<{ label: string }>;
}
type SourcePlan = { id: string } & ({ kind: 'folder'; root: string; path: string } | { kind: 'staging' });
interface SheetPlan {
  id: string; source: string; file: string; hash: string | null;
  read: { encoding: 'auto' | Encoding; delimiter: 'auto' | string; quote: '"' | "'" | 'none';
          headerLine: number; trim: boolean };
  rowKind: 'item' | 'media' | 'itemSet' | { column: ColumnRef };
  join: null | { on: 'identifier' | 'fileName'; column: ColumnRef;
                 normalize: { caseFold: boolean; leadingZeros: boolean; separators: boolean } };
  itemKey: ColumnRef | null;              // for media rows: the column naming their item
  columns: Array<{ column: ColumnRef; options: ColumnOptions }>;
}
type ColumnRef = { index: number; header: string };   // index wins; header is checked and shown
interface ColumnOptions { split: string | null; type: ValueType; lang: string | null; private: boolean }
type Encoding = 'utf-8' | 'utf-16le' | 'utf-16be' | 'windows-1252' | 'iso-8859-1';
type ValueType = 'literal' | 'uri' | { resource: 'identifier' | 'internalId' } | { dataType: string };
type SlotName = 'sets' | 'template' | 'class' | 'owner' | 'visibility' | 'sites' | 'mediaOrder'
  | 'ingester' | 'mediaSource' | 'itemKey' | Term;
type SlotRules = Partial<Record<SlotName, { sources: SourceRef[]; combine: 'first' | 'all';
                                            map: Record<string, string> }>>;   // value map, below
type SourceRef =
  | { from: 'group'; name: string }
  | { from: 'folder'; level: number | 'path' }       // 1 = the top folder under the source
  | { from: 'column'; sheet: string; column: ColumnRef }
  | { from: 'tag'; tag: TagId; files: 'first' | 'all' | { part: string } }
  | { from: 'file'; fact: 'name' | 'path' | 'size' | 'format' }   // format = the sniffed type
  | { from: 'constant'; value: string };
type TagId = 'DateTimeOriginal' | 'Artist' | 'Copyright' | 'ImageDescription' | 'GPS' | 'Model'
  | 'File name' | 'Folder' | 'Size' | 'Pixel dimensions' | 'Format';  // P8 adds IPTC and XMP ids
interface ActionPlan {
  action: 'create' | 'append' | 'revise' | 'update' | 'replace' | 'delete';
  match: Record<'item' | 'media' | 'itemSet', { key: Term | 'internalId'; scope: Scope }>;
  unmatched: 'create' | 'leave';
}
type Scope = 'importSets' | 'all' | { sets: number[] };
type FlagPolicy = 'keep' | 'repair' | 'leave';
interface NewType { label: string; from: 'mapping' | { starter: StarterId }; class: string | null }
```

The boundary refuses: duplicate source or sheet ids; refs to missing sources, sheets or columns;
`combine: all` on a single-valued slot; slot rules on a `delete` plan; duplicate `newTypes` labels
or labels of existing templates; `newTerms` that collide; path members with `..`, a NUL, a `\`, or
a leading `/`, before or after percent-decoding.

`importSets` means: for items, the sets this import's records name; for item sets, this import's
item-set records; for media, the media of the matched item.


### Boundary details (settled from P3-plan, 2026-10-20)

- `headerLine` is 1-based; `ColumnRef.index` is 0-based (position in the row).
- `delimiter` is one of `auto`, `,`, `;`, tab, `|`, `:`, space.
- A path member is refused when it has an empty segment (`a//b`), a `.` segment or a trailing `/`,
  so each file has one identity.
- Row keys are cell values: stored NFC and trimmed, never checked as paths.
- A policy given for a blocking kind is ignored.
- A `newTypes` label that an existing template has is refused on save (`refuse_existing_labels`,
  fed by the caller) and flagged `type-exists` on Check when the template appeared after the save.
- Case fold is lower-casing after trim and NFC (no full Unicode case fold; `ß` stays `ß`).
- Duplicate or empty headers are sheet notes, shown as anomalies; they are not a flag kind.
- Row keys: with a key column every key is `value#n` (`#1` included; an empty cell keys as `#n`);
  without one, the line number. A line whose cells are all empty is not a row.
- Extra cells that are all empty (a trailing delimiter) are dropped without a flag.
- Decisions that name missing sources or sheets are orphans, not refusals, so a plan that drops a
  source can still be saved.


### Derive details (settled from P3-derive, 2026-10-20)

- A null pattern makes every file an unmatched entry; sheets still make records.
- A `move … own` group never merges with a pattern group of the same text.
- `leave` on a file leaves its item record; on a row, the record the row made or joined.
- `importSets` for items and item sets is the union of existing sets any record of this import
  names; for media, the media of the matched item.
- `duplicate-row` also fires when two cells join one group; with `keep` the duplicate row is a
  record of its own. `unknown-kind` with `keep` reads the row as an item.
- A media row finds its file through a `fileName` join only; an `identifier` join on media rows is
  ignored. A media row whose item key names no item flags `unmatched`.
- `many-matches` with `keep` takes the lowest id. Match keys compare NFC and trimmed, no case fold.
- The value map applies after `combine`: a value mapped to `""` turns the slot off, it does not
  fall through to the next source. Two map keys that fold to one title are refused on save.
- Split pieces are trimmed silently; `space-at-ends` applies to unsplit values only.
- A tag rule with `files: first` reads the first file in media order; media slots resolve first.
- `encoding-suspect` with `repair` acts as `keep` until the sheet is read with another encoding.
- `empty-required` for a starter type uses the starter's fields (P3-wire passes them in the
  catalog).
- `map` may be missing in a stored plan and then means empty.


### Write details (settled from P3-writes, 2026-10-20)

- Slots have three states: unmapped, mapped and empty, mapped with values. `append` writes no
  single-valued slot (it adds sets and sites); `revise` writes slots with values; `update` writes
  every mapped slot (empty clears); `replace` also clears unmapped template, class, owner, sets and
  sites, and keeps visibility (the run always maps owner and visibility from the plan defaults).
- Append compares values by term, type, text and language (a URI by its URI, a link by its id).
- Removing a value removes its annotation; a restore makes the annotation again.
- The title is recomputed on every write, as upstream's title hydrator does.
- Pass-two link writes update the ledger row's `written` time, so the import's own link writes
  never count as touching its own records.


### Wire details (settled from P3-wire, 2026-10-20)

- A 409 answers the error document plus `code` (`stale`, `notDraft`, `v1Plan`, `typeExists`,
  `changed`, `blocked`, `nothing`, `confirm`, `busy`, `cannotRun`, `cannotUndo`, `notRunning`) and
  the members the admin needs (`revision`, `digest`, `blocking`, `deletes`, `status`, `importId`).
  A plan the boundary refuses answers 400.
- Run always rescans in the request; the scan route uses a job past 20,000 entries.
- The digest depends on who checks (a site admin's catalog holds only that admin as owner).
- History answers `{ total, page, perPage, items }`, 25 per page by default, at most 500.
- The sheet cache holds at most 16 reads and 128 MB.
- Walk anomalies per source live in `romeka_import_scan (import_id, source, anomalies, error,
  token)`, not in the entry table (0006 amended before it ever left the branch).
- Run starts with the revision it derived from (`start_run(id, revision)`), so a save between the
  derive and the start answers `changed`.
- The index loader reads entries unordered (`EntryRepo::all`) and sorts in Rust; no path index.


### What the admin screens need at scale (from the round-3 prototype, 2026-10-20)

The admin never draws more than about 1,000 elements: it reviews **shapes** and **causes**, pages
lists 50 rows at a time, and acts on whatever a filter selects. The server answers:

- **`shapes`** preview section: one row per shape with a count, three sample records (a seed in
  the request draws others), and `odd: bool`. A shape key is the file-count bucket (1, 2, 3, 4–6,
  7+) plus the set of side words in the record's file names. Side words are a parsed fact of each
  file entry: the pattern's `part` and `page` groups when the pattern has them, else a built-in list
  (`front`, `back`, `verso`, `recto`, `cover`, `पुढे`, `मागे`, `p<N>`, `page <N>`) matched as whole
  name parts. Unmatched files and likely copies are file shapes with counts and samples. A shape
  is odd when it holds under 2% of records or a file-count bucket of 7+.
- **Histogram** of files per record, in the buckets above, in `shapes`.
- **Cause samples**: `flags` adds, per kind and slot, three sample records (seeded like shapes) and,
  for `pair` suggestions, the row, its file name and the closest file name (edit distance ≤ 2
  after NFC and case fold).
- **Likely copies** at scan time: files of equal size whose names differ only by ` (2)`, ` copy`,
  `Copy of `, `-1` style suffixes, confirmed by a SHA-256 of both; an anomaly with a count.
- **Filters** on `records`: shape, folder, item set, item type, file-count bucket, flag kind and
  slot, left out or not, text in any slot. On `entries`: used or not, reason not used, folder,
  text. On sheet rows: paired or not.
- **Changes to many records** are decisions that name the records they hit, stored compactly:
  `{ kind: 'many', entries: EntryRef[], change: Decision-without-entry }`, applied in list order
  like other decisions. After a rescan, records that match the filter the change was made from
  but are not in `entries` are listed on Check ("12 new items match an earlier change") with an
  answer to extend it (the request carries the filter; the plan stores the result).
- **Spelling groups** for text slots that name things (template, sets, class): values grouped by
  title fold and edit distance ≤ 1, with a count per spelling; the admin turns a group into
  `SlotRule.map` entries.
- **Run progress** carries an estimate: records done, records total, and seconds left from the
  rate so far. No email: the page says the run goes on if the person leaves.
- Policies stay one per flag kind; a per-record `flag` decision covers exceptions.

## Values

A written value is `{ type, text, lang, label, isPublic }`:

- `literal`, with `lang` from the column, the decision, or none.
- `uri`: a cell `http://x.org/a Label` gives URI and label (upstream's rule).
- `resource`: names another resource by identifier (in the match scope, then among this import's
  records) or by internal id. Links to this import's own records are written in a second pass after
  every record exists, each pass-two write recorded in the ledger; cycles are allowed. Unresolved
  after both passes: `link-missing` (default `keep` as a literal).
- `dataType`: any data type the install knows, with that type's validation as flags.

`split` cuts a cell into values (upstream has one separator per import; v2 has one per column).

## Flags

Policies: `keep` writes the value or record as it is; `repair` applies the reader's automatic repair
(trim for `space-at-ends`, re-decode for `encoding-suspect` when the sheet's encoding is changed);
`leave` leaves the value, entry or record out. Only the blocking kinds stop a run; they are the
ones that would write broken data or have no defined result.

| Kind | On | Default | Blocking |
| --- | --- | --- | --- |
| `empty-required` | value | keep | no |
| `date-form` | value | keep | no |
| `space-at-ends` | value | repair | no |
| `encoding-suspect` (U+FFFD, or text that round-trips as 1252 mojibake) | value | keep | no |
| `name-encoding` | entry | keep | no |
| `cells-count` | row | leave | no |
| `no-file` | item record | keep | no |
| `not-media` | file | leave | no |
| `format-mismatch` | file | keep | no |
| `exists` | record | leave | no |
| `unmatched` | record | `action.unmatched` | no |
| `many-matches` | record | leave | no |
| `many-values` | value | keep (first) | no |
| `link-missing` | value | keep | no |
| `unknown-type` | record | keep | no |
| `unknown-kind` | row | leave | no |
| `duplicate-row` | row | leave | no |
| `unknown-term` | plan | — | yes |
| `template-missing` | plan | — | yes |
| `type-exists` | plan | — | yes |
| `encoding-unsupported` | sheet | — | yes, for that sheet |
| `sheet-unread` (missing, over 64 MB, or no header line) | sheet | — | yes, for that sheet |
| `unknown-owner` (owner value names no user) | record | keep (default owner) | no |
| `unknown-class` (class value names no class) | record | keep (no class) | no |

**Anomalies** are shown on Check and are not flags: records whose files span folders; records with
more than 4 times the median file count and more than 5 files; identifiers that exist in the install
outside the match scope; unmapped columns; orphans; `decision-overlap`; entries no record uses;
symlinks, dot entries and decoded names skipped by the walk; sheets whose bytes changed since their
hash was stored.

## Actions

The action is one per import (upstream parity). Each record kind follows it.

| Action | Values | Media | Sets, sites | Template, class, owner, visibility |
| --- | --- | --- | --- | --- |
| `create` | written | the record's media created | added | from slots |
| `append` | added (distinct by type, text, lang) | the record's new media added | added | unchanged |
| `revise` | each slot with values replaces that term's values; an empty slot leaves it | new media added | replaced when the slot has values | replaced when the slot has values |
| `update` | each mapped slot replaces that term's values; an empty slot clears it | new media added | replaced | replaced |
| `replace` | every value removed, then the slots written | new media added; existing media kept | replaced | replaced |
| `delete` | — | the matched resource and its media deleted | — | — |

No action except `delete` removes a media file, so every action except `delete` is undone by its
before-image plus deleting the media it added. `delete` is not undoable; the run requires `confirm`
equal to the number of resources it deletes.

## Lifecycle and the ledger

Statuses: `draft` → `running` → `done` | `run_failed`; `done` | `run_failed` | `undo_failed` →
`undoing` → `undone` | `undo_failed`. A dead job (no heartbeat for 2 minutes, or its job row ended)
is recorded on the first read as `run_failed` or `undo_failed`.

- **Frozen plan.** Run start copies the plan into `ran_plan`. A non-draft import's plan cannot
  change; edits go into a copy.
- **Digest.** Check returns the digest of the model it shows. Run takes the digest, rescans and
  derives again, and answers 409 `changed` if they differ. One run at a time install-wide, started
  in one locked transaction (SQLite `BEGIN IMMEDIATE`, MySQL `SELECT … FOR UPDATE`).
- **Per record.** Each record commits on its own, with its ledger rows (`romeka_import_resource`
  with the record key and the time written) and, for a changed resource, its before-image, in one
  transaction. Bytes reach the store before the commit and are removed if it fails. A record that
  fails is listed in `summary.failures` as `{ record, key, detail }` with a translatable key; the
  import ends `done` when any record committed, `run_failed` when none did.
- **Stop.** `POST /imports/{id}/stop` sets `stop_requested` (no heartbeat); the job stops after the
  record in hand and the import ends `run_failed`, so it can be retried or undone.
- **Retry.** A `run_failed` import, or a `done` import whose `summary.failures` is not empty, runs again from `ran_plan` against a rescan. Records whose key
  is in the ledger are skipped, so retry converges. Records with no stable key cannot be told
  apart: media rows and item-set rows get a key from their row key, files from their entry.
- **Copy.** `POST /imports { copyFrom }` makes a draft with the plan (decisions kept; those that no
  longer match evidence become orphans). Staging is not copied; files uploaded again under the same
  paths reattach their decisions.
- **left.csv** is written to the store when a run ends (`imports/<id>/left.csv`): the left-out and
  failed entries with `#reason`. It no longer depends on the folder staying put.
- **Undo** acts on one import that is `done`, `run_failed` or `undo_failed`. Order: restore changed
  resources from before-images; delete created media; delete created items; delete created item
  sets, templates and properties that nothing else uses. For each resource:
  - not touched: deleted (created) or restored (changed), through the API write paths;
  - touched: kept and listed with what touched it, unless `force` names it or says `all`. A forced
    restore loses the later change, which the confirmation states.
  - already gone: skipped and reported.
- **Touched** means, after the ledger row's written time: `modified` is later; a resource outside
  this import links to it; media were added outside this import; its set or site membership changed
  outside this import; a later import changed it (named in the kept list). P5-undo verifies that
  every write path updates `modified`, and fixes the ones that do not.
- **Liveness.** `romeka job run` refuses a job whose import heartbeat is younger than 2 minutes.
- **Plan saves** carry the plan's revision; a `PUT` with an old revision answers 409 `stale`.
- The v1 single undo window (`undoable`, `romeka_import_one_undoable`) is removed.

## Arrival

- **Archive roots.** `ROMEKA_ARCHIVE_ROOTS` holds `name=path` pairs separated by `;`. The v1
  `ROMEKA_ARCHIVE_ROOT` stays as the root named `archive`. Paths are made absolute at config load.
  Global admins and site admins may read every root (as v1).
- **Staging.** Each import owns `staging/<importId>/` in the store, never under the public `/files`
  prefix. Uploads use tus 1.0 core (create with `Upload-Length` and `Upload-Metadata`, `PATCH` with
  `Upload-Offset`, `HEAD`); a `PATCH` whose offset is not the stored offset answers 409. The file
  name in metadata passes the plan's path rules. Limits: `import_upload_max_file` (2 GB) and
  `import_upload_max_total` (50 GB per import), counting uploads in progress. Only the import's
  owner and global admins may upload.
- **ZIP.** A job extracts a ZIP into staging. It refuses entries whose normalised path leaves the
  staging area, absolute paths, symlink entries and nested extraction (a ZIP inside stays a file).
  `\` becomes `/`. Names without the UTF-8 flag decode as cp437. A second entry whose name equals an
  earlier one after case fold is renamed with `~n` and flagged. Extraction stops, and the upload is
  refused, when the extracted bytes would pass the import's limit, an entry's ratio passes 100:1
  with more than 10 MB out, or the entry count passes 100,000.
- **Retention.** A sweep deletes staging 30 days after an import reaches `done` or `undone`, 30 days
  after the last run attempt of a failed import, and 7 days after a draft was last saved; incomplete
  uploads go after 24 hours. The draft shows the date. Settings: `import_staging_keep_days`,
  `import_draft_staging_keep_days`.
- **URL sources.** The `ingester` and `mediaSource` slots drive the existing ingesters (`url`,
  `html`, `youtube`, `iiif`, `iiif_presentation`, `oembed`). Every fetch any ingester makes, in the
  import and in the API, goes through one guard: `http` and `https` only; no credentials in the
  URL; the host resolved once, every address checked, and the connection made to the checked
  address; refused ranges are loopback, link-local (incl. `169.254.169.254`), private, CGNAT, IPv6
  ULA and IPv4-mapped forms of these; each redirect (at most 5) checked the same way; also refused: NAT64 and 6to4 forms of refused
  addresses, `fec0::/10`, `0.0.0.0/8`, `192.0.0.0/24`, `198.18.0.0/15`, `240.0.0.0/4`; size capped at
  `import_upload_max_file`; 60 s to connect and between bytes, 1 hour per fetch; the body streams
  into the store (P6-urls). The operator may allow private ranges
  (`import_allow_private_urls`). Check never fetches.

## Storage and file access

- A `Store` trait in `omeka-media` carries every byte: `put` (bytes), `put_stream`, `read`,
  `read_range`, `delete`, `exists`, `list(prefix)`, and `url(path, Access)` with `Access::Public` or
  `Access::Signed { ttl }`. `FileStore` is the first implementation, an S3-compatible store the
  second. Thumbnailing and EXIF read through the trait.
- **Private files.** A file of a private resource is served only to a user who may see it. Locally,
  Caddy's `forward_auth` sends `/files/*` to `GET /api/romeka/files/allow` with the original URI;
  the server percent-decodes once, normalises, refuses traversal, and answers 204 or 403 from the
  session or an API key. The first version keeps no cache; P2-files measures the cost and adds one
  only with invalidation on every visibility, membership and undo write. Private files answer with
  `Cache-Control: private` and `Vary: Cookie`. Every `/files` answer sends `X-Content-Type-Options:
  nosniff`, and HTML and SVG files `Content-Security-Policy: sandbox`. On object storage, originals
  use signed URLs and the thumbnails of public resources public URLs.

## Database access

New import code in omeka-api is generic over `DB: omeka_db::Backend` from the start; the server and
CLI instantiate it with SQLite until P10. All SQL lives in omeka-db. P10 adds instantiation and
dialect work, not a rewrite of the importer.

## Performance

Release build, 7 runs, median, from a warm evidence index: a preview of 10,000 entries within
150 ms and of 40,000 within 600 ms (v1: 105 ms and 529 ms). A scan of 100,000 files within 30 s
on the NVMe. A unit that misses a figure reports it with the profile. The admin's initial JS stays
under the `perf:check` ceiling; a unit that would pass it stops and reports the bytes.

## Wire (romeka-only routes; rows in docs/compat.md)

All need a signed-in global admin or site admin.

| Route | Answer |
| --- | --- |
| `GET /api/romeka/archive?root=&path=` | `{ roots, root, path, entries }` |
| `GET /api/romeka/imports?page=&perPage=` | history, newest first |
| `POST /api/romeka/imports` `{ sources } \| { copyFrom }` | a draft with a default plan |
| `GET /api/romeka/imports/{id}` | the import, its plan and revision |
| `PUT /api/romeka/imports/{id}` `{ plan, revision }` | 409 `stale`, 409 unless draft |
| `DELETE /api/romeka/imports/{id}` | drafts only, 204 |
| `POST /api/romeka/imports/{id}/scan` | rescans; `{ jobId }` past 20,000 entries |
| `POST /api/romeka/imports/{id}/preview` `{ include, plan?, records?, entries? }` | sections below |
| `POST /api/romeka/imports/{id}/run` `{ digest, confirm? }` | `{ jobId }`; also a retry |
| `POST /api/romeka/imports/{id}/stop` | 202; the job stops after the record in hand |
| `POST /api/romeka/imports/{id}/undo` `{ force?: Array<{ kind, id }> \| 'all' }` | `{ jobId }` |
| `GET /api/romeka/imports/{id}/left.csv` | the stored file |
| `/api/romeka/imports/{id}/uploads[/{upload}]` | tus 1.0 core |
| `GET /api/romeka/starters` | the server's starter list |
| `GET /api/romeka/files/allow` | 204 or 403 |

Preview sections (`include`): `entries` (paged; each with its record and flags), `sheets` (per sheet:
encoding guess and confidence, delimiter, header line, raw lines, counts, column list), `tags`,
`records` (paged by kind; filter by flag kind, slot, or text in any slot), `flags` (counts per kind
and slot), `anomalies`, `orphans`, `check` (counts per action and kind, new sets, types, fields,
flags by policy, blocking reasons, `canRun`, `digest`).

## Command line

`romeka import run --plan plan.json --as <email> [--dry-run]`, `romeka import undo <id> [--force
all]`, `romeka import list`. They call the same omeka-api functions as the routes. `romeka import
csv` and `romeka import media` are removed in P7.

## Database (migration 0006, SQLite and MySQL; claimed 2026-10-20, romeka-s-d5 agreed)

No install holds an import worth keeping (8124: one draft that never ran, zero import resources,
measured 2026-10-20), so 0006 replaces the v1 import tables instead of converting them. It drops
`romeka_import_resource` and `romeka_import` and creates the v2 tables. MySQL gets the same
migration so a fresh MySQL install builds the v2 schema.

- `romeka_import (id, owner_id, sources TEXT, plan TEXT, ran_plan TEXT NULL, revision INTEGER NOT
  NULL DEFAULT 0, status, job_id, summary TEXT NULL, copied_from NULL, created, finished NULL)`,
  status CHECK over the v2 statuses, FKs as v1 (`ON DELETE SET NULL` for owner, job, copied_from).
  JSON members are LONGTEXT on MySQL.
- `romeka_import_resource (import_id, kind VARCHAR(32), target_id, record VARCHAR(512), change_kind
  VARCHAR(16) NOT NULL DEFAULT 'created', written DATETIME NOT NULL, PRIMARY KEY (import_id, kind,
  target_id))`, `ON DELETE CASCADE` from the import, an index on `(import_id, record)`.
- `romeka_import_before (import_id, kind VARCHAR(32), target_id, image, PRIMARY KEY (import_id,
  kind, target_id))`, `image` = JSON of values (with annotation values), template, class, owner, visibility, item sets,
  sites (with positions for item sets), media order and primary media, tagged with its kind.
- `romeka_import_upload (import_id, id VARCHAR(64), path, length BIGINT, received BIGINT, created)`
  (no `offset`: reserved in MySQL).
- `romeka_import_entry` (the evidence index): `(import_id, source VARCHAR(64), path, size,
  modified, media_type, flags)`, keyed by `(import_id, source, path_hash)` where `path_hash` is the
  SHA-256 of the path, computed in Rust (not a generated column), so MySQL's 3072-byte key limit
  never binds.
- On MySQL every key text column (`source`, `path`, `record`, upload `path`) is `utf8mb4_bin`, so
  `A.jpg` and `a.jpg` stay two keys. No column is named `offset` or `change` (reserved words).

The same unit deletes the v1 import code from `import2/base` (`crates/omeka-api/src/import/`, the
v1 routes in `crates/omeka-server/src/imports*`, `crates/omeka-cli/src/jobs/import_archive.rs`),
because it cannot compile against the new tables. The admin's v1 Import page stays until P4
replaces it; it has no routes in between. Main keeps v1 until v2 lands.

## Beyond upstream

Per-column separators; join normalisation and file-name joins; several sheets per import; folders,
staging uploads and URLs in one import; decisions that survive a new rule; flags with policies
instead of rejection; EDTF dates kept as written; undo of every action except delete; the touched
check; retry and copy; anomalies; protected files of private resources; ODS, TSV, UTF-16, Latin-1
and cp437 names (upstream reads UTF-8 only); one URL guard for every ingester.

## Scenarios (e2e and server tests)

Fixture folders are generated by scripts; no committed binary over 10 KB. Each row names its oracle.

| # | Evidence | Pass when |
| --- | --- | --- |
| S1 | the BHC_006 demo | 4 artifact items, 4 photo items, 1 scan item, all private; `_scratch` files flagged `not-media` or unmatched |
| S2 | Devanagari, Tamil and accented names, no sheet | one item per object; the example matches itself |
| S3 | flat folder, AF/PH code in the name, a register | template and set taken from the name part |
| S4 | `Box n/Folder m/0001.tif` | one item per folder pair; media in page order; first page primary |
| S5 | camera dump plus a sheet with a file-name column | rows join by file name |
| S6 | register with one row per image | media rows attach by item key |
| S7 | UTF-16 TSV, `\|`-split values, `Title_en` and `Title_hi` | values split, languages set |
| S8 | mixed sheet: item sets, items, media | three kinds created and linked, links by identifier |
| S9 | rerun of S1 with a fixed register, action `revise` | values change; undo restores them |
| S10 | undo after a cataloguer edits one item | that item kept and listed; the rest undone |
| S11 | upload a ZIP with a cp437 name, an escaping path and a bomb entry | name decoded; escaping entry refused; bomb refused |
| S12 | URL rows: public, loopback, redirect to loopback, DNS name to a private address | public ingested; the rest refused |
| S13 | delete action | `confirm` required; not undoable |
| S14 | private import, then fetch a file signed out, signed in, by API key | 403, 200, 200 |
| S15 | S1, decisions made, then a new pattern | decisions on surviving files still apply; others listed as orphans |
| S16 | re-sorted register with value-keyed and line-keyed decisions | value-keyed kept; line-keyed orphaned |
| S17 | kill the job mid-run, then retry | the import ends `done` with no duplicate |
| S18 | CR-only CSV, duplicate headers, an unclosed quote | rows read; both columns mappable; the bad row flagged |
| S19 | symlinked folder, dot files, Latin-1 names | counted as anomalies; names decoded and flagged |
| S20 | two admins save one draft | the second save answers 409 `stale` |

## Units

Owner paths are enforced by review: a diff outside its unit's paths is sent back.

v2 server code is built in a new module, `crates/omeka-api/src/import2/`; P3-db deletes the v1 module on `import2/base`, and I-close renames `import2` to `import` when every unit has landed (a rename earlier would conflict with every unit in flight). In the table, `import2/…` means that module. Main keeps v1 and receives v2 only when P4, P5 and P6 pass together.

| Unit | Owner paths | Depends on |
| --- | --- | --- |
| P0-compiler, P0-jobs, P0-media, P0-starters, P0-docs | their briefs | |
| P2-store | `crates/omeka-media/src/store*`, every `FileStore` call site | P0 landed |
| P2-files | `crates/omeka-server/src/files_allow*`, `deploy/Caddyfile`, the `/files` headers | P2-store |
| P2-urlguard | `crates/omeka-media/src/http*`, every ingester's fetch | P2-store |
| P3-db | `crates/omeka-db/**` (migration 0006, repos), deleting v1 import code in api, server and cli | |
| P3-plan | `import2/{plan,values,decisions}*` | |
| P3-evidence | `import2/{archive,evidence,sheet}*`, `crates/omeka-server/src/config*` roots | P3-plan, P3-db |
| P3-derive | `import2/{derive,grouping,join,slots,matching,flags}*` | P3-evidence |
| P3-wire | `crates/omeka-server/src/imports*`, `import2/{preview,check}*`, the cutover | P3-derive |
| P3-run | `import2/{run,ledger}*`, `crates/omeka-cli/src/jobs/import*` | P3-derive, P3-db, P2-store |
| P5-undo | `import2/undo*`, `modified` fixes in write paths | P3-run |
| P5-actions | `import2/actions*` | P5-undo |
| P4-api | `web/admin/src/api/import*` | P3-wire |
| P4-evidence | `web/admin/src/pages/import/{home,draft,folder,files,example*}*` | P4-api |
| P4-slots | `web/admin/src/pages/import/{sets,types}*` | P4-api |
| P4-sources | `web/admin/src/pages/import/sources*` | P4-api |
| P4-grid | `web/admin/src/pages/import/grid*` | P4-api |
| P4-check-run | `web/admin/src/pages/import/{check,run,undo,history}*`, `web/admin/src/pages/jobs/**` | P4-api, P5-undo |
| P6-upload | `crates/omeka-server/src/uploads*`, `import2/staging*`, `crates/omeka-cli/src/jobs/{unzip,sweep}*`, `web/admin/src/pages/import/upload*` | P2-store, P3-wire |
| P7-cli | `crates/omeka-cli/src/commands/import*` | P5-actions |
| P8-readers | ODS reader, IPTC and XMP tags, more thumbnail formats | P3-evidence |
| P9-s3 | `crates/omeka-media/src/store_s3*`, signed URLs | P2-files |
| P10-sql | workspace: raw SQL into omeka-db, `Backend` through api, server and cli, MySQL backup and search, `serve --database-url`, MySQL e2e | P7-cli |
| I-locales | `web/admin/src/i18n/**` | each P4 unit |
| I-e2e | `web/admin/e2e/**`, fixture scripts | P4, P5, P6 |
| I-close | `docs/import-v1-assumptions.md` "Closed by", `docs/compat.md` | all |

## Decisions (log)

| Date | Decision | Why |
| --- | --- | --- |
| 2026-10-20 | Redesign: rules propose, decisions correct; flags not gates | affordance read; user |
| 2026-10-20 | Upstream parity plus extras; one importer; mixed rows | user |
| 2026-10-20 | Undo any import, touched resources kept; before-images for all but delete | user |
| 2026-10-20 | Private by default; files of private resources protected | user |
| 2026-10-20 | Store trait, then importer, then MySQL runtime (in scope) | user |
| 2026-10-20 | Decisions key on entries (files by source and path; rows by key value, else line) | identifiers change with the rule; line keys break on a re-sort |
| 2026-10-20 | Group key and identifier are different things; the join uses the group key | the identifier slot is resolved after the join |
| 2026-10-20 | Action per import; match key and scope per record kind | upstream parity; a per-row delete is too easy to make by accident |
| 2026-10-20 | `replace` never removes media | keeps every non-delete action undoable from before-images |
| 2026-10-20 | Separate `run_failed` and `undo_failed`; frozen `ran_plan`; digest check at run | retry, undo and drift each need an unambiguous state |
| 2026-10-20 | Evidence index cached per import | previews on large roots; the digest needs a fixed input |
| 2026-10-20 | Several archive roots by name | cheap once roots are named; v1 root keeps the name `archive` |
| 2026-10-20 | tus 1.0 core for uploads | standard, resumable, small client |
| 2026-10-20 | One URL guard for every ingester fetch, import and API | the url ingester has no guard today; hosted installs |
| 2026-10-20 | No `fix` policy: `keep`, `repair`, `leave`; four blocking plan kinds | the user's "flags, never gates"; blocking only where data would break |
| 2026-10-20 | One destructive migration, no v1 conversion; v1 import code deleted on the branch | no install holds imports (measured); user: no users to migrate |
| 2026-10-20 | Keep upstream's default file-validation lists (they refuse `.nef`, `.heic`, `.flac`, `.svg`, `.json`, `.mkv`, `.gz`); `not-media` on Check names the refused type and links to Settings > Security | parity; the operator widens the list, the import says how |
| 2026-10-20 | Slot value maps (also turn a set off); stop route; retry of a `done` import with failures | gaps found by the P4 prototype |
| 2026-10-20 | MySQL 0003 edited to drop a CHECK that MySQL 8.0 refuses (error 3823); no MySQL install exists yet, so the checksum change breaks nothing | the MySQL 8.0 suite had been skipping every test since 0003 |
| 2026-10-20 | Admin UX: a guided first pass (Your files, What we found, Your list, Where they go, Check, Add) that lands in seven tabs renamed Files, List, Item sets, Item types, All items, Check, Add; the word "item" with a gloss; the server card hidden when no root is set up for the user; plain words enforced by proto2/GLOSSARY.md and glossary-check.js | user (prototype round 2) |
| 2026-10-20 | Contract additions: `.xlsx`/`.xls` sheets (P8); a `pair` decision (row ↔ file by hand); likely copies by content hash for files of equal size (an anomaly) | user (prototype round 2) |
| 2026-10-20 | The UX works on shapes and causes, not single items, at 50 to 40,000 items | user: "the ux breaks as soon as you have more than 50 items" |
| 2026-10-20 | UX at scale: shapes and causes, paged lists, changes to many that name their records; server additions listed under "What the admin screens need at scale" | user; round-3 prototype measured flat 20/21 decisions from 50 to 40,000 items |
| 2026-10-20 | Contract review: Sonnet adversarial pass found the identity cycle, line anchors, shared `failed`, undefined actions and security gaps; all folded in | second opinion |

## Run log

| Unit | Verdict | Evidence |
| --- | --- | --- |
| P0-docs | verified (claims checked against code) | import2/p0-docs |
| P0-media | unit-test-verified | 40 sniffed formats against libmagic 5.46 on the oracle; 422 bodies match upstream; 1115 tests |
| P3-fixes | unit-test-verified | 2,007 workspace tests; 391 MySQL tests each on 8.0 and MariaDB 11; 9 messages rewritten, 0 glossary hits |
| P3-wire | unit-test-verified | 1,953 workspace tests; S1 and S20 through the routes; digest and label mutations caught; preview 10k ~100 ms, 40k ~575 ms on a loaded machine |
| P3-writes | unit-test-verified | 319 SQLite, 379 each on MySQL 8.0.46 and MariaDB 11.8.9; 24-case apply/restore property test; 5 mutations caught |
| P3-derive | unit-test-verified | 149 import2 tests, 16 mutations (2 survivors fixed), 10k 95-116 ms, 40k 444-528 ms on a loaded machine |
| P3-db | unit-test-verified | 1,782 workspace tests; 346 MySQL tests each on mysql 8.0 and MariaDB 11; start race 20/20; guards removed fail their tests |
| P3-evidence | unit-test-verified | 79 import2 tests, 917 api+server tests, 5 mutations caught, 100k walk median 387 ms warm |
| P3-plan | unit-test-verified | 38 import2 tests, 378 omeka-api tests, clippy clean |
| P0-compiler | unit-test-verified | 24/24 example tests, 711 admin tests, rg check of compiled patterns, perf 85,387 B |
