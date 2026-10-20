/**
 * `POST /api/romeka/imports/{id}/preview`: the derived view of a draft. The
 * server is the only place the plan's rules run; the admin asks for the
 * sections and pages it shows, and this module parses what comes back.
 */
import type { ImportPlan } from './imports';
import {
  asBoolean,
  asNumber,
  asOneOf,
  asRecord,
  asRecordOf,
  asString,
  asStringList,
  field,
  nullableField,
  optionalField,
  parseListOf,
  ParseError,
} from './parse';
import type { Parser, Path } from './types';

export type PreviewSection = 'files' | 'sets' | 'csv' | 'tags' | 'rows' | 'check';
export type FileFilter = 'all' | 'match' | 'miss';
export type PerItem = 1 | 2 | 3;
export type ProblemKind = 'empty' | 'date' | 'space' | 'encoding' | 'cells' | 'nofile';
const PROBLEM_KINDS: readonly ProblemKind[] = ['empty', 'date', 'space', 'encoding', 'cells', 'nofile'];

/** Why the server cannot derive the view; any of them stops a run. */
export type PreviewErrorKey =
  | 'patternMissing'
  | 'patternSyntax'
  | 'patternIdentifier'
  | 'csvParse'
  | 'delimiter'
  | 'headerRow'
  | 'joinColumn'
  | 'csvUnreadable'
  | 'templateMissing'
  | 'termUnknown'
  | 'typeExists';
export const PREVIEW_ERROR_KEYS: readonly PreviewErrorKey[] = [
  'patternMissing',
  'patternSyntax',
  'patternIdentifier',
  'csvParse',
  'delimiter',
  'headerRow',
  'joinColumn',
  'csvUnreadable',
  'templateMissing',
  'termUnknown',
  'typeExists',
];

export interface FilesQuery {
  readonly filter: FileFilter;
  readonly perItem?: PerItem;
  readonly offset: number;
  readonly limit: number;
}

export interface RowsQuery {
  readonly problemsOnly: boolean;
  /** Only the rows whose identifier an item already holds. */
  readonly existingOnly?: boolean;
  readonly kind?: ProblemKind;
  readonly query?: string;
  /** With `kind`, narrows the kind to one field. */
  readonly term?: string | null;
  readonly offset: number;
  readonly limit: number;
}

export interface PreviewRequest {
  readonly include: readonly PreviewSection[];
  readonly files?: FilesQuery;
  readonly rows?: RowsQuery;
  /**
   * The plan to derive from instead of the stored one. The admin sends its
   * unsaved plan, so a preview never waits on the debounced save, and a
   * candidate plan, so the Folder tab can count what a change would match.
   */
  readonly plan?: ImportPlan;
}

export interface FileRow {
  readonly path: string;
  readonly matched: boolean;
  /** The item's 1-based number in the preview, or `null` for an unmatched file. */
  readonly item: number | null;
  readonly itemFiles: number;
  /** Group name to `[start, end)`, UTF-16 offsets into `path`. */
  readonly groups: Readonly<Record<string, readonly [number, number]>>;
}

export interface FilesPreview {
  readonly total: number;
  readonly matched: number;
  readonly items: number;
  readonly groupNames: readonly string[];
  readonly buckets: Readonly<Partial<Record<'1' | '2' | '3', number>>>;
  readonly biggest: { readonly identifier: string; readonly files: number } | null;
  readonly filtered: number;
  readonly page: readonly FileRow[];
}

export interface SetPreview {
  readonly key: string;
  readonly name: string;
  readonly use: boolean;
  readonly matched: number;
  readonly total: number;
  readonly existing: boolean;
}

export interface CsvPreview {
  readonly files: readonly string[];
  readonly head: readonly string[];
  readonly rawLines: readonly string[];
  readonly detected: { readonly delimiter: string; readonly headerRow: number };
  readonly cellsLookLike1252: number;
  readonly cellsWithSpaces: number;
  /** Why the rows have the wrong cell counts, when they do. */
  readonly rowCountError: 'delimiter' | 'headerRow' | null;
  readonly joined: number;
  readonly rowsWithoutFile: number;
  readonly itemsWithoutRow: number;
}

export interface TagPreview {
  readonly tag: string;
  readonly kind: 'exif' | 'file';
  readonly found: number;
  readonly sample: string;
}

export interface RowCell {
  readonly term: string;
  readonly value: string;
  readonly source: 'edit' | 'csv' | 'file' | 'name' | '';
  readonly problems: readonly ProblemKind[];
  readonly conflict?: { readonly csv: string; readonly files: string };
}

export interface RowProblem {
  readonly kind: 'cells' | 'nofile';
  readonly line?: number;
  readonly cells?: number;
}

export interface RowPreview {
  readonly identifier: string;
  readonly sets: readonly string[];
  readonly type: string;
  readonly files: readonly string[];
  readonly cells: readonly RowCell[];
  readonly extra: readonly string[];
  readonly rowProblems: readonly RowProblem[];
  readonly skipped: boolean;
  readonly problems: number;
  /** The item that already holds this identifier; the run never creates the row. */
  readonly existing: { readonly itemId: number } | null;
}

export interface RowsPreview {
  readonly columns: readonly { readonly term: string; readonly label: string; readonly sources: readonly string[] }[];
  readonly unmapped: readonly string[];
  readonly total: number;
  readonly withProblems: number;
  /** Rows whose identifier an item already holds. */
  readonly existing: number;
  readonly filtered: number;
  readonly kinds: readonly { readonly kind: ProblemKind; readonly term: string | null; readonly count: number }[];
  readonly page: readonly RowPreview[];
}

export interface CheckPreview {
  readonly create: number;
  readonly files: number;
  readonly newSets: readonly string[];
  readonly existingSets: readonly string[];
  readonly newTypes: readonly string[];
  readonly newFields: readonly string[];
  readonly withProblems: number;
  readonly skipped: number;
  /** Rows the run skips because an item already holds their identifier. */
  readonly existing: number;
  readonly unmatchedFiles: number;
  readonly unmatchedSample: readonly string[];
  readonly canRun: boolean;
  readonly openUndo: { readonly id: number; readonly folder: string; readonly items: number } | null;
}

export interface Preview {
  readonly error?: { readonly key: PreviewErrorKey; readonly detail?: string };
  readonly files?: FilesPreview;
  readonly sets?: readonly SetPreview[];
  readonly csv?: CsvPreview;
  readonly tags?: readonly TagPreview[];
  readonly rows?: RowsPreview;
  readonly check?: CheckPreview;
}

type Fields = Record<string, unknown>;
const text = (object: Fields, key: string, path: Path): string =>
  asString(field(object, key, path), [...path, key]);
const count = (object: Fields, key: string, path: Path): number =>
  asNumber(field(object, key, path), [...path, key]);
const flag = (object: Fields, key: string, path: Path): boolean =>
  asBoolean(field(object, key, path), [...path, key]);
const strings = (object: Fields, key: string, path: Path): readonly string[] =>
  asStringList(field(object, key, path), [...path, key]);
const kindOf = (value: unknown, path: Path): ProblemKind => asOneOf(PROBLEM_KINDS, value, path);

const parseSpan = (raw: unknown, path: Path): readonly [number, number] => {
  const [start, end, ...rest] = parseListOf(asNumber)(raw, path);
  if (start === undefined || end === undefined || rest.length > 0 || start > end) {
    throw new ParseError(path, '[start, end]');
  }
  return [start, end];
};

const parseFileRow: Parser<FileRow> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    path: text(object, 'path', path),
    matched: flag(object, 'matched', path),
    item: nullableField(object, 'item', path, asNumber),
    itemFiles: count(object, 'itemFiles', path),
    groups: asRecordOf(field(object, 'groups', path), [...path, 'groups'], parseSpan),
  };
};

const parseBuckets = (raw: unknown, path: Path): FilesPreview['buckets'] => {
  const object = asRecord(raw, path);
  const buckets: Partial<Record<'1' | '2' | '3', number>> = {};
  for (const key of ['1', '2', '3'] as const) {
    const value = optionalField(object, key, path, asNumber);
    if (value !== undefined) {
      buckets[key] = value;
    }
  }
  return buckets;
};

const parseFiles: Parser<FilesPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    total: count(object, 'total', path),
    matched: count(object, 'matched', path),
    items: count(object, 'items', path),
    groupNames: strings(object, 'groupNames', path),
    buckets: parseBuckets(field(object, 'buckets', path), [...path, 'buckets']),
    biggest: nullableField(object, 'biggest', path, (value, at) => {
      const biggest = asRecord(value, at);
      return { identifier: text(biggest, 'identifier', at), files: count(biggest, 'files', at) };
    }),
    filtered: count(object, 'filtered', path),
    page: parseListOf(parseFileRow)(field(object, 'page', path), [...path, 'page']),
  };
};

const parseSet: Parser<SetPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    key: text(object, 'key', path),
    name: text(object, 'name', path),
    use: flag(object, 'use', path),
    matched: count(object, 'matched', path),
    total: count(object, 'total', path),
    existing: flag(object, 'existing', path),
  };
};

const parseCsv: Parser<CsvPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const detected = asRecord(field(object, 'detected', path), [...path, 'detected']);
  const at = [...path, 'detected'];
  return {
    files: strings(object, 'files', path),
    head: strings(object, 'head', path),
    rawLines: strings(object, 'rawLines', path),
    detected: { delimiter: text(detected, 'delimiter', at), headerRow: count(detected, 'headerRow', at) },
    cellsLookLike1252: count(object, 'cellsLookLike1252', path),
    cellsWithSpaces: count(object, 'cellsWithSpaces', path),
    rowCountError: nullableField(object, 'rowCountError', path, (value, at) =>
      asOneOf(['delimiter', 'headerRow'], value, at),
    ),
    joined: count(object, 'joined', path),
    rowsWithoutFile: count(object, 'rowsWithoutFile', path),
    itemsWithoutRow: count(object, 'itemsWithoutRow', path),
  };
};

const parseTag: Parser<TagPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    tag: text(object, 'tag', path),
    kind: asOneOf(['exif', 'file'], field(object, 'kind', path), [...path, 'kind']),
    found: count(object, 'found', path),
    sample: text(object, 'sample', path),
  };
};

const parseCell: Parser<RowCell> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const conflict = optionalField(object, 'conflict', path, (value, at) => {
    const entry = asRecord(value, at);
    return { csv: text(entry, 'csv', at), files: text(entry, 'files', at) };
  });
  const cell = {
    term: text(object, 'term', path),
    value: text(object, 'value', path),
    source: asOneOf(['edit', 'csv', 'file', 'name', ''], field(object, 'source', path), [...path, 'source']),
    problems: parseListOf(kindOf)(field(object, 'problems', path), [...path, 'problems']),
  };
  return conflict === undefined ? cell : { ...cell, conflict };
};

const parseRowProblem: Parser<RowProblem> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const line = optionalField(object, 'line', path, asNumber);
  const cells = optionalField(object, 'cells', path, asNumber);
  return {
    kind: asOneOf(['cells', 'nofile'], field(object, 'kind', path), [...path, 'kind']),
    ...(line === undefined ? {} : { line }),
    ...(cells === undefined ? {} : { cells }),
  };
};

const parseRow: Parser<RowPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    identifier: text(object, 'identifier', path),
    sets: strings(object, 'sets', path),
    type: text(object, 'type', path),
    files: strings(object, 'files', path),
    cells: parseListOf(parseCell)(field(object, 'cells', path), [...path, 'cells']),
    extra: strings(object, 'extra', path),
    rowProblems: parseListOf(parseRowProblem)(field(object, 'rowProblems', path), [...path, 'rowProblems']),
    skipped: flag(object, 'skipped', path),
    problems: count(object, 'problems', path),
    existing: nullableField(object, 'existing', path, (value, at) => ({
      itemId: count(asRecord(value, at), 'itemId', at),
    })),
  };
};

const parseRows: Parser<RowsPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    columns: parseListOf((value, at) => {
      const column = asRecord(value, at);
      return { term: text(column, 'term', at), label: text(column, 'label', at), sources: strings(column, 'sources', at) };
    })(field(object, 'columns', path), [...path, 'columns']),
    unmapped: strings(object, 'unmapped', path),
    total: count(object, 'total', path),
    withProblems: count(object, 'withProblems', path),
    existing: count(object, 'existing', path),
    filtered: count(object, 'filtered', path),
    kinds: parseListOf((value, at) => {
      const kind = asRecord(value, at);
      return {
        kind: kindOf(field(kind, 'kind', at), [...at, 'kind']),
        term: nullableField(kind, 'term', at, asString),
        count: count(kind, 'count', at),
      };
    })(field(object, 'kinds', path), [...path, 'kinds']),
    page: parseListOf(parseRow)(field(object, 'page', path), [...path, 'page']),
  };
};

const parseCheck: Parser<CheckPreview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    create: count(object, 'create', path),
    files: count(object, 'files', path),
    newSets: strings(object, 'newSets', path),
    existingSets: strings(object, 'existingSets', path),
    newTypes: strings(object, 'newTypes', path),
    newFields: strings(object, 'newFields', path),
    withProblems: count(object, 'withProblems', path),
    skipped: count(object, 'skipped', path),
    existing: count(object, 'existing', path),
    unmatchedFiles: count(object, 'unmatchedFiles', path),
    unmatchedSample: strings(object, 'unmatchedSample', path),
    canRun: flag(object, 'canRun', path),
    openUndo: nullableField(object, 'openUndo', path, (value, at) => {
      const open = asRecord(value, at);
      return { id: count(open, 'id', at), folder: text(open, 'folder', at), items: count(open, 'items', at) };
    }),
  };
};

const parseError = (raw: unknown, path: Path): NonNullable<Preview['error']> => {
  const object = asRecord(raw, path);
  const detail = optionalField(object, 'detail', path, asString);
  const key = asOneOf(PREVIEW_ERROR_KEYS, field(object, 'key', path), [...path, 'key']);
  return detail === undefined ? { key } : { key, detail };
};

/** Only the sections the request asked for are present. */
export const parsePreview: Parser<Preview> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const error = optionalField(object, 'error', path, parseError);
  const files = optionalField(object, 'files', path, parseFiles);
  const sets = optionalField(object, 'sets', path, parseListOf(parseSet));
  const csv = optionalField(object, 'csv', path, parseCsv);
  const tags = optionalField(object, 'tags', path, parseListOf(parseTag));
  const rows = optionalField(object, 'rows', path, parseRows);
  const check = optionalField(object, 'check', path, parseCheck);
  return {
    ...(error === undefined ? {} : { error }),
    ...(files === undefined ? {} : { files }),
    ...(sets === undefined ? {} : { sets }),
    ...(csv === undefined ? {} : { csv }),
    ...(tags === undefined ? {} : { tags }),
    ...(rows === undefined ? {} : { rows }),
    ...(check === undefined ? {} : { check }),
  };
};
