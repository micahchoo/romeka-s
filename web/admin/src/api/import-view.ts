/**
 * One import as the routes answer it, the history, the archive listing and the
 * small answers of the job routes. The shapes are `ImportView` in
 * `crates/omeka-server/src/imports/wire.rs` and its sibling modules.
 */
import type { ImportId, ImportPlan, SourcePlan } from './import-plan';
import { parseSourcesOf } from './import-plan-read';
import { asCount, asFlag, list, member, nullable, oneOf, strict } from './import-read';
import { parseSourceState } from './import-sections';
import { parseUndoReport } from './import-undo';
import type { UndoReport } from './import-undo';
import type { SourceState } from './import-sections';
import { ParseError, asString, brandId } from './parse';
import type { Parser, Path } from './types';

export type ImportStatus =
  | 'draft'
  | 'running'
  | 'done'
  | 'run_failed'
  | 'undoing'
  | 'undone'
  | 'partly_undone'
  | 'undo_failed';
export const IMPORT_STATUSES: readonly ImportStatus[] = [
  'draft',
  'running',
  'done',
  'run_failed',
  'undoing',
  'undone',
  'partly_undone',
  'undo_failed',
];

/** A record the run could not write: its ledger key, the message's catalogue key, and its English text. */
export interface RunFailure {
  readonly record: string;
  readonly key: string;
  readonly detail: string;
}

/** What a run keeps: progress, the failures, and whether it stopped on request. */
export interface RunSummary {
  readonly progress: { readonly done: number; readonly total: number };
  readonly failures: readonly RunFailure[];
  readonly stopped: boolean;
}

/** Records done of the total, and the seconds left at the rate so far (`null` until one is done, and once ended). */
export interface RunProgress {
  readonly done: number;
  readonly total: number;
  readonly secondsLeft: number | null;
}

export interface ImportRow {
  readonly id: ImportId;
  readonly ownerId: number | null;
  readonly status: ImportStatus;
  /** The plan's revision: a save sends the one it read. */
  readonly revision: number;
  readonly sources: readonly SourcePlan[];
  readonly created: string;
  readonly finished: string | null;
  readonly summary: RunSummary | null;
  /** The undo job's report; `null` until an undo has run. */
  readonly undoReport: UndoReport | null;
  /** When a reader found the job dead (it stopped beating), or `null`. */
  readonly died: string | null;
  readonly stopRequested: boolean;
  readonly jobId: number | null;
  readonly copiedFrom: number | null;
  /** Present while a run writes its progress. */
  readonly progress?: RunProgress;
  /** How often the server went on with the run by itself after a restart; absent until it did. */
  readonly resumed?: number;
}

/** One import with its plan, as every route but the history answers it. */
export interface ImportDetail extends ImportRow {
  readonly plan: ImportPlan;
  /** The plan the first run froze; absent for a draft. */
  readonly ranPlan?: ImportPlan;
}

export const ROW_KEYS = [
  'id',
  'ownerId',
  'status',
  'revision',
  'sources',
  'created',
  'finished',
  'summary',
  'undoReport',
  'died',
  'stopRequested',
  'jobId',
  'copiedFrom',
];

const parseFailure: Parser<RunFailure> = (raw, path = []) => {
  const object = strict(raw, path, ['record', 'key', 'detail']);
  return {
    record: member(object, 'record', path, asString),
    key: member(object, 'key', path, asString),
    detail: member(object, 'detail', path, asString),
  };
};

const parseDoneOfTotal = (raw: unknown, path: Path): { readonly done: number; readonly total: number } => {
  const object = strict(raw, path, ['done', 'total']);
  return { done: member(object, 'done', path, asCount), total: member(object, 'total', path, asCount) };
};

const parseSummary: Parser<RunSummary> = (raw, path = []) => {
  const object = strict(raw, path, ['progress', 'failures', 'stopped']);
  return {
    progress: member(object, 'progress', path, parseDoneOfTotal),
    failures: member(object, 'failures', path, list(parseFailure)),
    stopped: member(object, 'stopped', path, asFlag),
  };
};

const parseProgress: Parser<RunProgress> = (raw, path = []) => {
  const object = strict(raw, path, ['done', 'total', 'secondsLeft']);
  return {
    done: member(object, 'done', path, asCount),
    total: member(object, 'total', path, asCount),
    secondsLeft: member(object, 'secondsLeft', path, nullable(asCount)),
  };
};

/** The members every import answer has; `import-detail-read.ts` adds the plan. */
export const rowFields = (object: Record<string, unknown>, path: Path): ImportRow => {
  const row: ImportRow = {
    id: brandId<'ImportId'>(member(object, 'id', path, asCount)),
    ownerId: member(object, 'ownerId', path, nullable(asCount)),
    status: member(object, 'status', path, oneOf(IMPORT_STATUSES)),
    revision: member(object, 'revision', path, asCount),
    sources: member(object, 'sources', path, parseSourcesOf),
    created: member(object, 'created', path, asString),
    finished: member(object, 'finished', path, nullable(asString)),
    summary: member(object, 'summary', path, nullable(parseSummary)),
    undoReport: member(object, 'undoReport', path, nullable(parseUndoReport)),
    died: member(object, 'died', path, nullable(asString)),
    stopRequested: member(object, 'stopRequested', path, asFlag),
    jobId: member(object, 'jobId', path, nullable(asCount)),
    copiedFrom: member(object, 'copiedFrom', path, nullable(asCount)),
  };
  const withProgress =
    object['progress'] === undefined ? row : { ...row, progress: member(object, 'progress', path, parseProgress) };
  return object['resumed'] === undefined
    ? withProgress
    : { ...withProgress, resumed: member(object, 'resumed', path, asCount) };
};

/** A row of the history: no plan. */
export const parseImportRow: Parser<ImportRow> = (raw, path = []) =>
  rowFields(strict(raw, path, ROW_KEYS, ['progress', 'resumed']), path);

/** Why a filled `Files` cell made no pair (`preview/pairs.rs`, `Why`). */
export type PairsWhy = 'noRow' | 'noFile' | 'manyFiles' | 'noItem';
export const PAIRS_WHY: readonly PairsWhy[] = ['noRow', 'noFile', 'manyFiles', 'noItem'];

/** A filled cell that made no pair: the row cell and the Files cell as the list gave them. */
export interface UnmatchedCell {
  readonly row: string;
  readonly text: string;
  readonly why: PairsWhy;
}

/** What `POST …/pairs` did, with the import after it. */
export interface PairsAdded {
  /** Pair decisions the list added. */
  readonly added: number;
  /** Cells whose pair the plan already held. */
  readonly already: number;
  /** The first cells that made no pair; `unmatchedTotal` counts them all. */
  readonly unmatched: readonly UnmatchedCell[];
  readonly unmatchedTotal: number;
  readonly import: ImportDetail;
}

export interface ArchiveEntry {
  readonly kind: 'folder' | 'file';
  readonly name: string;
  /** Relative to the root: what a folder source's `path` takes. */
  readonly path: string;
  readonly size: number | null;
}

export interface ArchiveListing {
  /** Every configured root, by name, in configured order. */
  readonly roots: readonly string[];
  /** `null` when no root is configured. */
  readonly root: string | null;
  readonly path: string;
  readonly entries: readonly ArchiveEntry[];
}

export const parseArchiveListing: Parser<ArchiveListing> = (raw, path = []) => {
  const object = strict(raw, path, ['roots', 'root', 'path', 'entries']);
  const entry = (value: unknown, at: Path): ArchiveEntry => {
    const each = strict(value, at, ['kind', 'name', 'path', 'size']);
    return {
      kind: member(each, 'kind', at, oneOf<'folder' | 'file'>(['folder', 'file'])),
      name: member(each, 'name', at, asString),
      path: member(each, 'path', at, asString),
      size: member(each, 'size', at, nullable(asCount)),
    };
  };
  return {
    roots: member(object, 'roots', path, list(asString)),
    root: member(object, 'root', path, nullable(asString)),
    path: member(object, 'path', path, asString),
    entries: member(object, 'entries', path, list(entry)),
  };
};

export interface History {
  readonly total: number;
  readonly page: number;
  readonly perPage: number;
  readonly items: readonly ImportRow[];
}

export const parseHistory: Parser<History> = (raw, path = []) => {
  const object = strict(raw, path, ['total', 'page', 'perPage', 'items']);
  return {
    total: member(object, 'total', path, asCount),
    page: member(object, 'page', path, asCount),
    perPage: member(object, 'perPage', path, asCount),
    items: member(object, 'items', path, list(parseImportRow)),
  };
};

export interface JobStart {
  readonly jobId: number;
}

export const parseJobStart: Parser<JobStart> = (raw, path = []) => {
  const object = strict(raw, path, ['jobId']);
  return { jobId: member(object, 'jobId', path, asCount) };
};

/** A scan inside the request answers the sources; past 20,000 entries it is a job. */
export type ScanResult =
  | { readonly kind: 'scanned'; readonly sources: readonly SourceState[] }
  | { readonly kind: 'job'; readonly jobId: number };

export const parseScanResult: Parser<ScanResult> = (raw, path = []) => {
  if (typeof raw === 'object' && raw !== null && 'jobId' in raw) {
    return { kind: 'job', jobId: parseJobStart(raw, path).jobId };
  }
  const object = strict(raw, path, ['sources']);
  if (object['sources'] === undefined) {
    throw new ParseError(path, 'sources or a job');
  }
  return { kind: 'scanned', sources: member(object, 'sources', path, list(parseSourceState)) };
};
