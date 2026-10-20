# Archive import: contract and run log

Branch `admin/archive-import`, worktree `../romeka-s-wt/archive-import`. Uncommitted until the user
decides. Merges to main go through `flock ../.romeka-main.lock`. Migration 0005 is claimed with
session romeka-s-d5 (2026-10-19).

The design comes from a throwaway prototype (seven tabs, mark-an-example grouping, problem kinds,
undo until the next import). This file is the contract every unit builds against. A unit that needs
to change the contract reports the change; it does not drift silently.

## Definition of done

1. With `ROMEKA_ARCHIVE_ROOT` set, the admin shows an Import page under Content. Its seven tabs
   (Folder, Item sets, Item types, Sources, Grid, Check, Run) drive a real import of a server folder.
2. The import creates items, item sets, values and media (originals plus thumbnails, TIFF included),
   runs as a job, and the Jobs page shows its log and refreshes while it runs.
3. The last import can be undone until the next import starts (global window). Undo deletes the
   imported items, media files and memberships, and the new sets, item types and fields that nothing
   else uses.
4. An e2e spec drives the whole flow and the undo on a fixture folder.
5. Gates pass: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
   admin `typecheck`, `lint`, `test`, `build`, `e2e`, `perf:check`. New admin strings exist in every
   locale (Spanish by hand, Indic through `scripts/translate-locales.mjs`).

## Schema (migration 0005, SQLite and MySQL)

Both tables carry a `romeka_` prefix: they are not upstream, and `import` is a risky name in MySQL.

```sql
CREATE TABLE romeka_import (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  owner_id  INTEGER NULL REFERENCES user (id) ON DELETE SET NULL,
  folder    TEXT NOT NULL,          -- relative to ROMEKA_ARCHIVE_ROOT
  plan      TEXT NOT NULL,          -- ImportPlan JSON
  status    TEXT NOT NULL,          -- draft | running | done | failed | undoing | undone
  undoable  INTEGER NOT NULL DEFAULT 0,
  job_id    INTEGER NULL REFERENCES job (id) ON DELETE SET NULL,
  summary   TEXT NULL,              -- ImportSummary JSON, written as the run goes
  created   DATETIME NOT NULL,
  finished  DATETIME NULL
);
-- At most one import has an open undo window.
CREATE UNIQUE INDEX romeka_import_one_undoable ON romeka_import (undoable) WHERE undoable = 1;

CREATE TABLE romeka_import_resource (
  import_id INTEGER NOT NULL REFERENCES romeka_import (id) ON DELETE CASCADE,
  kind      TEXT NOT NULL,          -- item | media | item_set | resource_template | property
  target_id INTEGER NOT NULL,
  PRIMARY KEY (import_id, kind, target_id)
);
```

MySQL has no partial index: the repository enforces the single open window in the same
transaction that sets it. Memberships need no rows: `item_item_set` cascades from the item.

## The plan (stored JSON, sent by the admin)

camelCase keys. The server parses it at the boundary and refuses unknown shapes.

```ts
interface ImportPlan {
  pattern: string | null;            // regex in the JS ∩ Rust `regex` subset; null until set
  example: { path: string; labels: Record<string, Label>; optional: Label[] } | null; // UI state, kept
  setLevel: 'level1' | 'level2' | 'both' | 'path' | 'group';
  sets: Record<string, { use: boolean; name: string }>;           // keyed by folder key
  typeFor: Record<string, number | { starter: StarterId; name: string }>; // set key -> template id or new
  csv: null | {
    file: string;                    // relative to the import folder
    delimiter: ',' | ';' | '\t';
    headerRow: number;               // 1-based line
    windows1252: boolean;
    trim: boolean;
    joinColumn: string;
    columns: Record<string, string | { newField: string }>; // header -> term, or a new local field label
  };
  meta: Array<{ tag: string; term: string; on: 'item' | 'media' | 'both'; from: 'first' | 'part' | 'all'; part?: string }>;
  priority: 'csv' | 'files';
  edits: Record<string, Record<string, string>>;   // identifier -> term -> value
  skip: string[];
  withoutFiles: string[];
  csvRow: Record<string, 'keep' | 'ignore'>;       // identifier -> choice for a CSV line with the wrong cell count
  problemRows: 'leave' | 'fix';
}
type Label = 'identifier' | 'part' | 'page' | 'set';
type StarterId = 'letter' | 'photograph' | 'postcard' | 'diary' | 'person' | 'other';
```

The example compiler lives in the admin (`pages/import/example.ts`). The server only receives
`pattern` and checks that it compiles with the `regex` crate and has an `identifier` group.

## Wire (romeka-only routes; add rows to docs/compat.md)

All need a signed-in global admin or site admin (the roles that may write items, sets, templates
and vocabularies). Errors use the existing error document. Routes live in
`crates/omeka-server/src/imports.rs`; logic lives in `crates/omeka-api/src/import/`.

| Route | Answer |
| --- | --- |
| `GET /api/romeka/archive?path=` | `{ enabled, path, entries: [{ name, kind: 'dir'\|'file', files? }] }`. `enabled: false` and no entries when `ROMEKA_ARCHIVE_ROOT` is unset. A path that escapes the root is 400. |
| `GET /api/romeka/imports` | history, newest first: `[{ id, folder, status, undoable, created, finished, summary }]` |
| `POST /api/romeka/imports` `{ folder }` | a new draft `{ id, folder, status, plan, ... }` with a default plan |
| `GET /api/romeka/imports/{id}` | the import with its plan |
| `PUT /api/romeka/imports/{id}` `{ plan }` | saves a draft's plan; 409 unless status is draft |
| `DELETE /api/romeka/imports/{id}` | deletes a draft; 409 otherwise |
| `POST /api/romeka/imports/{id}/preview` `{ include, files?, rows? }` | the derived view, below |
| `POST /api/romeka/imports/{id}/run` | `{ jobId }`. 409 when another import is running or undoing, or the draft's check finds nothing to create. Closes the previous undo window and opens this one in one transaction. |
| `POST /api/romeka/imports/{id}/undo` | `{ jobId }`. 409 unless this import holds the open window and is done or failed. |
| `GET /api/romeka/jobs/{id}/log` | `text/plain`, the job's log so far |

`preview` takes `include: Array<'files'|'sets'|'csv'|'tags'|'rows'|'check'>` and paging:
`files: { filter: 'all'|'match'|'miss', perItem?: 1|2|3, offset, limit }`,
`rows: { problemsOnly, kind?, query?, offset, limit }` (limit at most 200). It answers only the
sections asked for:

```ts
interface Preview {
  error?: { key: string; detail?: string };        // e.g. an invalid pattern
  files?: { total: number; matched: number; items: number; groupNames: string[];
            buckets: { '1'?: number; '2'?: number; '3'?: number };
            biggest: { identifier: string; files: number } | null;
            filtered: number;
            page: Array<{ path: string; matched: boolean; item: number | null; itemFiles: number;
                          groups: Record<string, [number, number]> }> };   // UTF-16 code-unit offsets into path, so JS can slice
  sets?: Array<{ key: string; name: string; use: boolean; matched: number; total: number; existing: boolean }>;
  csv?: { files: string[]; head: string[]; rawLines: string[]; detected: { delimiter: string; headerRow: number };
          cellsLookLike1252: number; cellsWithSpaces: number; rowCountError: string | null;
          joined: number; rowsWithoutFile: number; itemsWithoutRow: number };
  tags?: Array<{ tag: string; kind: 'exif' | 'file'; found: number; sample: string }>;
  rows?: { columns: Array<{ term: string; label: string; sources: string[] }>; unmapped: string[];
           total: number; withProblems: number; filtered: number;
           kinds: Array<{ kind: ProblemKind; term: string | null; count: number }>;
           page: Array<{ identifier: string; sets: string[]; type: string; files: string[];
                         cells: Array<{ term: string; value: string; source: 'edit'|'csv'|'file'|'name'|'';
                                        problems: ProblemKind[]; conflict?: { csv: string; files: string } }>;
                         extra: string[]; rowProblems: Array<{ kind: 'cells' | 'nofile'; line?: number; cells?: number }>;
                         skipped: boolean; problems: number }> };
  check?: { create: number; files: number; newSets: string[]; existingSets: string[]; newTypes: string[];
            newFields: string[]; withProblems: number; skipped: number; unmatchedFiles: number;
            unmatchedSample: string[]; canRun: boolean; openUndo: { id: number; folder: string; items: number } | null };
}
type ProblemKind = 'empty' | 'date' | 'space' | 'encoding' | 'cells' | 'nofile';
```

`ImportSummary` (also in the history list): `{ items, files, thumbnails, left, newSets, newTypes,
newFields, progress: { done, total } }`.

### Contract amendments from B3a (2026-10-19)

- `rows` paging also takes `term?: string | null`; with `kind` it narrows the kind to one field.
- `csv.rowCountError` is a key: `"delimiter"`, `"headerRow"` or null.
- `columns[].sources` entries are `"type"`, `"csv:<header>"` or `"tag:<tag>"`; the admin translates them.
- `error.key` is one of `patternMissing`, `patternSyntax`, `patternIdentifier`, `csvParse`, `delimiter`,
  `headerRow`, `joinColumn`, `csvUnreadable`, `templateMissing`; any of them makes `canRun` false.
- A new field's term is `local:` + lower camel case of its label (any script's letters and digits).
  The admin reads the term from `columns`, never derives it.
- Starter field lists live on the server (`import/catalog.rs`); an item with no `typeFor` entry gets
  the catalog's default template (Base Resource).
- Date checks cover every Dublin Core date term and refuse impossible days. Dot files and non-UTF-8
  names are skipped, folder symlinks are never followed, the plan's CSV is not an import file, only
  the first CSV row per identifier joins, and a files page holds at most 1000 entries.
- Rust `regex`: lookaround and backreferences are syntax errors; `\d` and `\w` are Unicode-aware.

### Contract amendments from F1 (2026-10-19), for B4 to implement

- The preview body takes an optional `plan: ImportPlan`; when present the server derives from it
  instead of the stored plan (edits ahead of the debounced PUT, and candidate patterns for the
  optional-part hint). `files.limit: 0` is valid and answers counts with an empty page.
- `sets[]` entries gain `items: number`, the items whose first set is this one.
- Admin strings for the import live in the `import` namespace with keys prefixed `archive.`.

### Contract amendments from B3b (2026-10-19)

- `ImportSummary` gains `failed`; `left` counts skipped, left-out and failed rows.
- The `local` vocabulary is never recorded, so undo leaves it in place.
- An import never reruns. Only a draft may run; after that it can only be undone.
- `UndoReport { items, media, itemSets, templates, fields, kept: [{ kind, id }] }`.

### Contract amendments from B4 (2026-10-19)

- Preview error keys gain `termUnknown` and `typeExists` (detail: the term or the type name).
- The import JSON carries `jobId`. DELETE answers 204. With the feature off, create, preview and
  run answer 404.
- A stale run (status running, job error) shows as `failed` on the wire and can be undone.
- `romeka job run` takes `--base-url` and `--archive-root`; `romeka serve` takes `--store` and
  loads the config itself. Tag reads are cached per process by path, mtime and size.

### Contract amendments from I1a (2026-10-19)

- `ImportSummary` gains `itemSetIds`, `existingSets` and `failures: [{ identifier, reason }]`.
- The stored summary keeps the undo report as `undo`; the import JSON answers it as `undoReport`
  (null until an undo ends), in the detail and the history list.
- `GET /api/romeka/imports/{id}/left.csv`: `dcterms:identifier`, the grid terms and `reason`, one
  line per skipped, left-out or failed row.

### Contract amendments from J (2026-10-19)

- A run or undo whose job row has ended, or whose heartbeat (summary `heartbeat`, else its job's
  start) is older than 2 minutes, is dead. The first read records it: import `failed` with
  `finished`, job `error`. The import JSON answers `died` (ISO time or null) and never `heartbeat`.
- `run` and `undo` start through `ImportRepo::start_run` / `start_undo`, one locked transaction
  each (`BEGIN IMMEDIATE` on SQLite, `SELECT ... FOR UPDATE` on MySQL). The per-process mutex is gone.

### Contract amendments from R (2026-10-19)

- Rows gain `existing: { itemId } | null`; rows paging gains `existingOnly`; rows, check and the
  summary gain `existing`. Existing rows are never created and never count as problems; left.csv
  gives them the reason `exists`, which wins over every other reason.

## Jobs

- Classes `Romeka\Job\ImportArchive` and `Romeka\Job\UndoImport`, args `{ "importId": n }`.
- The server starts jobs through a `JobDispatch` trait object in `AppState`, supplied by the CLI's
  `serve` (it re-executes `romeka job run <id> --database <path> --store <dir>`). Tests supply an
  in-process dispatcher. The server never imports omeka-cli.
- The job appends progress lines to `job.log` and updates `romeka_import.summary`.
- Run: the open undo window moves to this import at the start, so a partial run can be undone. Each
  row (item, values, set links, media) commits on its own and records its `romeka_import_resource`
  rows in the same transaction. Files are stored before the row commits and removed if it fails.
- Undo: one item at a time through the API delete path (`write/items.rs` delete, which removes media
  files), deleting the `romeka_import_resource` row as each target goes, so a rerun converges. Then
  new item sets with no items, new templates no resource uses, and new properties no value uses.
  A new field that is in use stays, and the log says so.

## Decisions (log)

| Date | Decision | Why |
| --- | --- | --- |
| 2026-10-19 | Seven separate tabs | user |
| 2026-10-19 | Undo window is global | user |
| 2026-10-19 | Grouping by marking an example; regex as the expert mode | user, affordance analysis |
| 2026-10-19 | Server is the only validator; the browser renders pages of rows | one implementation of the rules |
| 2026-10-19 | CSV parsed and repaired on the server, Windows-1252 by a hand table | the CSV is in the server folder; no new dependency |
| 2026-10-19 | EXIF through `kamadak-exif` in omeka-media; TIFF thumbnails via the `image` crate's `tiff` feature | prototype showed TIFF masters render blank |
| 2026-10-19 | Logic in `omeka-api/src/import`, routes in omeka-server, jobs registered in omeka-cli | omeka-cli depends on omeka-server |
| 2026-10-19 | `romeka import csv` and `import media` stay as they are; only the CSV reader moves to omeka-api | different input shape; keeps the run bounded |
| 2026-10-19 | Commits: none until the user asks | user's standing rule; earlier runs left verified work uncommitted |

## Units

| Unit | Owner paths | Depends on | Status |
| --- | --- | --- | --- |
| B1 db | `crates/omeka-db/**` | | |
| B2 media | `crates/omeka-media/**` | | |
| B3a import read side (plan, archive, matching, CSV, preview) | `crates/omeka-api/src/import/**`, `crates/omeka-api/src/lib.rs`, `crates/omeka-cli/src/csv.rs` move | B1, B2 | |
| B3b import write side (run, undo) | `crates/omeka-api/src/import/**` | B3a | |
| B4 server, jobs | `crates/omeka-server/**`, `crates/omeka-cli/**` | B3 | |
| F1 admin: shell, Folder, Item sets, Item types | `web/admin/src/pages/import/**`, `web/admin/src/api/**`, `web/admin/src/app/pages.tsx` | contract | |
| F2 admin: Sources, Grid | `web/admin/src/pages/import/**` | F1 | |
| F3 admin: Check, Run, undo, Jobs log | `web/admin/src/pages/import/**`, `web/admin/src/pages/jobs/**` | F2 | |
| I1a wire gaps | server import JSON + admin api | F3, B5 | |
| I1b e2e, docs, final gates | e2e, docs, READMEs | I1a | |

## Follow-up run (2026-10-19, "do all")

| Decision | Why |
| --- | --- |
| Re-import skips identifiers that already exist as items; no update mode | undo cannot restore overwritten values |
| A job is dead when its import heartbeat is older than 2 minutes while it claims to run | survives container restarts and pid reuse; no schema change (heartbeat in summary JSON) |
| Start of run and undo is one conditional UPDATE in omeka-db | replaces the per-process mutex; works on MySQL without a partial index |

| Unit | Owner paths | Wave |
| --- | --- | --- |
| J job lifecycle | omeka-db imports repo, omeka-server imports*, omeka-cli jobs/import_archive.rs, admin run/undo status text | 1 |
| R re-import skip | omeka-api import (catalog, derive, check, preview), admin check/grid strings | 1 |
| S small fixes | omeka-api import (tags, run), omeka-media image_metadata, omeka-cli csv import | 2 |
| O orphan sweep | omeka-cli new command, omeka-media store listing | 2 |
| P preview at scale | omeka-api import + omeka-server preview cache | 3 |

## Run log

(one row per unit as it lands: verdict, evidence)

| Unit | Verdict | Evidence |
| --- | --- | --- |
| J job lifecycle | VERIFIED | Heartbeat every 10 s, dead after 2 min, settle_dead records it; atomic start_run/start_undo (race test: 20 rounds, one winner; fails with plain BEGIN); MySQL 8 + MariaDB 11 pass; admin `run.died`; left.csv uses load_for_import. Coordinator rerun: db+server+cli 890 passed, 1 failure from S's in-flight `#reason` rename (S told). |
| O orphan sweep | VERIFIED | `romeka media sweep` dry run default, `--delete`, 10 min age floor, symlinks never entered; 3 tests. |
| S small fixes | VERIFIED | EXIF via seeks (12.96 MB TIFF: 12,960,173 -> 40,960 bytes read; 4 MB JPEG: 4,001,072 -> 32,768; bounded by one buffer refill per seek), template title property (Person -> foaf:name), one value per file, `#` columns ignored and left.csv `#reason` re-imports with `romeka import csv`. |
| Final e2e (coordinator) | FIXED | First final run: 94 passed, 3 failed. Root cause: J's start_run writes a heartbeat-only summary; the wire stripped it to `{}`; the admin parser requires `progress` and refused the answer, so the Run tab never opened. Fix: the wire answers `summary: null` when nothing but internal members remain (unit test failed first: Some({}) vs None). The second spec assumed overlapping identifiers that the fixture does not have (LT_002_front vs LT_002); corrected to 4 new items. The site-items failure was leftover items from the failed spec. Lesson: per-side fixtures hid a wire mismatch only the e2e joins. |
| P preview at scale | VERIFIED, no cache | Release, 7 runs: 10,000 items ~105 ms median per admin preview (Sources 149 ms), 1,000 items ~5 ms; limiter one core in the derive (callgrind: derive 58%, malloc/free ~35%); 40,000 items reaches 529 ms, so a model cache is the next step past ~25,000 items. Date problems checked per written value (test failed first). run.rs, run_tests.rs, CLI import.rs split under 600 lines. Harness: omeka-server/tests/imports_preview_scale.rs (ignored). |
| Wave 1-2 gates (coordinator) | VERIFIED | cargo test --workspace 1,775 passed, clippy and fmt clean. |
| R re-import skip | VERIFIED | Catalog.existing (one query), rows marked existing (never created, linked, excluded from problems), check/summary `existing`, left.csv reason `exists`; mutation checks. Coordinator rerun: omeka-api 336, admin 693. Amendments: rows `existing: {itemId}|null`, paging `existingOnly`, rows/check/summary `existing`. Integration in J: left.csv uses load_for_import. |
| Disk | ACTION | Root filesystem hit 100% (214 MB free); removed my finished units' cargo targets (139 GB); later units build under ../.cargo-targets on the RAID. |
| Final gates (coordinator) | VERIFIED | fmt, clippy, cargo test --workspace 1,750 passed, typecheck, lint, vitest 691, build, e2e 97 passed (with the Jobs-page log step), perf:check 85,403 B of 87,040. All five definition-of-done items verified. |
| Demo deploy | NOT RUN | Blocked by the session's permission check before any step. Script: scratchpad deploy-archive.sh; demo archive at ../romeka-archive-demo. |
| I1b e2e, docs, gates | VERIFIED | e2e spec drives seven tabs, run, Items page with TIFF thumbnail, undo, and second-import-closes-undo on a real server and job process; 97 e2e passed twice. One product fix: nav strip rescrolls when the Import link appears (shell.tsx MutationObserver; mutation-checked). Docs, compat rows, CHANGELOG, packaging files (not deployed). `left.csv` reason column is refused by `romeka import csv`: documented. Coordinator added a Jobs-page log step to the spec (DoD 2). |
| I1a wire gaps | VERIFIED | undoReport, itemSetIds/existingSets/failures, left.csv route, one history fetch, 204 pinned; mutation checks on three tests. Coordinator rerun: api, server, cli 926 passed; admin 691; clippy and typecheck clean. Open: whether `romeka import csv` accepts the extra `reason` column. |
| F3 Check + Run + undo + Jobs log | VERIFIED | Check dry run, Run with polling, undo confirmation and report, undo banner, Jobs log with 2 s refresh (mutation check on both poll stops). Coordinator rerun: 686 tests, build pass; initial JS 85,330 B. Wire gaps listed for I1a. |
| B5 fixes | VERIFIED | UndoReport.media counts removed media (test failed first: 0 vs 4); config.rs split 625 -> 538. Coordinator rerun: api, server, cli 921 passed. |
| B4 server + jobs | VERIFIED | Routes in omeka-server/src/imports.rs, JobDispatch seam, ROMEKA_ARCHIVE_ROOT, CLI job classes, job log route; real smoke: serve + child job imported 2 items/3 files and undid them (0 store files left). Coordinator rerun: db, api, cli, server 1,203 passed; workspace clippy clean. Open: a killed job process leaves its job in_progress and the import busy. |
| F2 Sources + Grid | VERIFIED | Sources (CSV detection, repairs, join, tag mapping, priority) and Grid (problem kinds with bulk fixes as one table, pages of 50, edit bar, Map… with new fields named by the server, CSV download built client-side). Coordinator rerun: 676 tests, build pass; initial JS 85,228 B, largest chunk 7,066 B. Not yet run against a live server. |
| B3b write side | VERIFIED | `import::run` / `import::undo` (8 tests on a real FileStore with JPEG and TIFF; mutation check failed 3 tests), transaction-level inserts added to omeka-db repos. Coordinator rerun: omeka-db, api, cli, server 1,186 passed; workspace clippy clean. Follow-ups: whole-file EXIF read twice per file; orphan files on a crash between store and commit; check should flag unknown terms and existing template labels. |
| F1 admin shell + tabs 1-3 | VERIFIED | Import page, seven tabs (4-7 placeholders), example compiler (`[0-9]` classes; Rust regex accepted the output via rg), file paging/filters/buckets, sets, types, nav link hidden when the archive is off (shell.tsx edited), import namespace in 9 locales. Coordinator rerun: typecheck, lint, 668 tests, build pass; initial JS 85,192 B of 87,040. Hazard: run `npx vitest --clearCache` after adding a globbed file. |
| B3a read side | VERIFIED | `omeka-api/src/import/` (4,367 lines, 54 tests), CSV reader moved with `romeka import/export csv` unchanged. Coordinator rerun: omeka-api + omeka-cli 423 passed, clippy clean. Amendments recorded above. Follow-up: the tag reader reads whole files (slow on large TIFFs). |
| B1 db | VERIFIED | Migration 0005 (SQLite + MySQL; MySQL skips 0004 by design, measured on MySQL 8 and MariaDB 11), ImportRepo, ImportResourceRepo::record on a caller's tx. Coordinator rerun: omeka-db 283 passed, clippy clean. Extra: CHECK constraints, close_undo_window, delete_draft. MySQL single-window rule is transaction-only (no partial index). |
| B2 media | VERIFIED | `image_metadata(bytes)` in omeka-media (kamadak-exif 0.6.1, BSD-2-Clause; +4 crates); TIFF thumbnails. Coordinator rerun: omeka-media 148 tests passed, clippy clean. Compat row proposed for TIFF thumbnails (I1). |
| Baseline (bf2aecc) | VERIFIED green | clippy workspace exit 0; cargo test workspace 1659 passed, 0 failed (102 binaries); admin typecheck and vitest exit 0 |
