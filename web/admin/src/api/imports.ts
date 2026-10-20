/**
 * The archive import (`/api/romeka/archive`, `/api/romeka/imports`), Romeka's
 * own routes: the wire types, their parsers and the calls. Only the Import page
 * reads it, so it stays out of the initial payload. The shapes are the contract
 * in `docs/archive-import.md`; the preview's derived view is `import-preview.ts`.
 */
import type { ApiClient } from './client';
import { parsePreview } from './import-preview';
import type { Preview, PreviewRequest } from './import-preview';
import {
  asBoolean,
  asIdNumber,
  asNumber,
  asOneOf,
  asRecord,
  asRecordOf,
  asString,
  asStringList,
  brandId,
  field,
  nullableField,
  optionalField,
  parseListOf,
} from './parse';
import type { Id, ItemSetId, Parser, Path, TemplateId } from './types';

export type ImportId = Id<'ImportId'>;

/** What one marked part of the example file name means. */
export type Label = 'identifier' | 'part' | 'page' | 'set';
export const LABELS: readonly Label[] = ['identifier', 'part', 'page', 'set'];

/** The item type starters the Item types tab offers (`pages/item-types/starters.ts`). */
export type StarterId = 'letter' | 'photograph' | 'postcard' | 'diary' | 'person' | 'other';
export const STARTER_IDS: readonly StarterId[] = ['letter', 'photograph', 'postcard', 'diary', 'person', 'other'];

/** Which folders become item sets. */
export type SetLevel = 'level1' | 'level2' | 'both' | 'path' | 'group';
export const SET_LEVELS: readonly SetLevel[] = ['level1', 'level2', 'both', 'path', 'group'];

export type ImportStatus = 'draft' | 'running' | 'done' | 'failed' | 'undoing' | 'undone';
const STATUSES: readonly ImportStatus[] = ['draft', 'running', 'done', 'failed', 'undoing', 'undone'];

/** The marked example the Folder tab compiles to `pattern`, kept so the marks reopen. */
export interface ImportExample {
  readonly path: string;
  /** Chip key (`d0` a folder, `n2` a name part) to its label. */
  readonly labels: Readonly<Record<string, Label>>;
  readonly optional: readonly Label[];
}

/** A new item type: a starter's fields under a name. */
export interface NewType {
  readonly starter: StarterId;
  readonly name: string;
}

export type TypeChoice = TemplateId | NewType;

export interface CsvPlan {
  readonly file: string;
  readonly delimiter: ',' | ';' | '\t';
  readonly headerRow: number;
  readonly windows1252: boolean;
  readonly trim: boolean;
  readonly joinColumn: string;
  readonly columns: Readonly<Record<string, string | { readonly newField: string }>>;
}

export interface MetaRule {
  readonly tag: string;
  readonly term: string;
  readonly on: 'item' | 'media' | 'both';
  readonly from: 'first' | 'part' | 'all';
  readonly part?: string;
}

export interface ImportPlan {
  readonly pattern: string | null;
  readonly example: ImportExample | null;
  readonly setLevel: SetLevel;
  /** Keyed by folder key. */
  readonly sets: Readonly<Record<string, { readonly use: boolean; readonly name: string }>>;
  /** Set key to an existing template or a new type. */
  readonly typeFor: Readonly<Record<string, TypeChoice>>;
  readonly csv: CsvPlan | null;
  readonly meta: readonly MetaRule[];
  readonly priority: 'csv' | 'files';
  /** Identifier to term to value. */
  readonly edits: Readonly<Record<string, Readonly<Record<string, string>>>>;
  readonly skip: readonly string[];
  readonly withoutFiles: readonly string[];
  readonly csvRow: Readonly<Record<string, 'keep' | 'ignore'>>;
  readonly problemRows: 'leave' | 'fix';
}

export interface ImportSummary {
  readonly items: number;
  readonly files: number;
  readonly thumbnails: number;
  /** Existing, skipped, left out for problems, and failed rows. */
  readonly left: number;
  readonly failed: number;
  /** Rows skipped because an item already holds their identifier. */
  readonly existing: number;
  readonly newSets: readonly string[];
  readonly newTypes: readonly string[];
  readonly newFields: readonly string[];
  /** The item sets the run created or added items to. */
  readonly itemSetIds: readonly ItemSetId[];
  /** The titles of the existing item sets the run added items to. */
  readonly existingSets: readonly string[];
  readonly progress: { readonly done: number; readonly total: number };
}

export type KeptKind = 'itemSet' | 'template' | 'field';
const KEPT_KINDS: readonly KeptKind[] = ['itemSet', 'template', 'field'];

/** What an undo removed (`UndoReport` in `omeka-api/src/import/undo.rs`). */
export interface UndoReport {
  readonly items: number;
  readonly media: number;
  readonly itemSets: number;
  readonly templates: number;
  readonly fields: number;
  /** What the import made that something else now uses, so the undo left it. */
  readonly kept: readonly { readonly kind: KeptKind; readonly id: number }[];
}

/** One row of the import history. */
export interface ImportRecord {
  readonly id: ImportId;
  readonly folder: string;
  readonly status: ImportStatus;
  readonly undoable: boolean;
  readonly created: string;
  readonly finished: string | null;
  readonly summary: ImportSummary | null;
}

/** One import with its plan. */
export interface ImportDetail extends ImportRecord {
  readonly plan: ImportPlan;
  readonly jobId: number | null;
  /** Null until an undo ends. */
  readonly undoReport: UndoReport | null;
  /** When the server found the import's job dead (it stopped beating), or null. */
  readonly died: string | null;
}

export interface ArchiveEntry {
  readonly name: string;
  readonly kind: 'dir' | 'file';
  /** How many files a folder holds, when the server counted them. */
  readonly files?: number;
}

export interface ArchiveListing {
  readonly enabled: boolean;
  readonly path: string;
  readonly entries: readonly ArchiveEntry[];
}

const text = (object: Record<string, unknown>, key: string, path: Path): string =>
  asString(field(object, key, path), [...path, key]);
const count = (object: Record<string, unknown>, key: string, path: Path): number =>
  asNumber(field(object, key, path), [...path, key]);
const flag = (object: Record<string, unknown>, key: string, path: Path): boolean =>
  asBoolean(field(object, key, path), [...path, key]);
const strings = (object: Record<string, unknown>, key: string, path: Path): readonly string[] =>
  asStringList(field(object, key, path), [...path, key]);

const parseExample: Parser<ImportExample> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    path: text(object, 'path', path),
    labels: asRecordOf(field(object, 'labels', path), [...path, 'labels'], (value, at) =>
      asOneOf(LABELS, value, at),
    ),
    optional: parseListOf((value, at) => asOneOf(LABELS, value, at))(
      field(object, 'optional', path),
      [...path, 'optional'],
    ),
  };
};

const parseTypeChoice = (raw: unknown, path: Path): TypeChoice => {
  if (typeof raw === 'number') {
    return brandId<'TemplateId'>(asIdNumber(raw, path));
  }
  const object = asRecord(raw, path);
  return {
    starter: asOneOf(STARTER_IDS, field(object, 'starter', path), [...path, 'starter']),
    name: text(object, 'name', path),
  };
};

const parseCsvColumn = (raw: unknown, path: Path): string | { readonly newField: string } =>
  typeof raw === 'string' ? raw : { newField: text(asRecord(raw, path), 'newField', path) };

const parseCsvPlan: Parser<CsvPlan> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    file: text(object, 'file', path),
    delimiter: asOneOf([',', ';', '\t'], field(object, 'delimiter', path), [...path, 'delimiter']),
    headerRow: count(object, 'headerRow', path),
    windows1252: flag(object, 'windows1252', path),
    trim: flag(object, 'trim', path),
    joinColumn: text(object, 'joinColumn', path),
    columns: asRecordOf(field(object, 'columns', path), [...path, 'columns'], parseCsvColumn),
  };
};

const parseMetaRule: Parser<MetaRule> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const part = optionalField(object, 'part', path, asString);
  const rule = {
    tag: text(object, 'tag', path),
    term: text(object, 'term', path),
    on: asOneOf(['item', 'media', 'both'], field(object, 'on', path), [...path, 'on']),
    from: asOneOf(['first', 'part', 'all'], field(object, 'from', path), [...path, 'from']),
  };
  return part === undefined ? rule : { ...rule, part };
};

export const parseImportPlan: Parser<ImportPlan> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    pattern: nullableField(object, 'pattern', path, asString),
    example: nullableField(object, 'example', path, parseExample),
    setLevel: asOneOf(SET_LEVELS, field(object, 'setLevel', path), [...path, 'setLevel']),
    sets: asRecordOf(field(object, 'sets', path), [...path, 'sets'], (value, at) => {
      const set = asRecord(value, at);
      return { use: flag(set, 'use', at), name: text(set, 'name', at) };
    }),
    typeFor: asRecordOf(field(object, 'typeFor', path), [...path, 'typeFor'], parseTypeChoice),
    csv: nullableField(object, 'csv', path, parseCsvPlan),
    meta: parseListOf(parseMetaRule)(field(object, 'meta', path), [...path, 'meta']),
    priority: asOneOf(['csv', 'files'], field(object, 'priority', path), [...path, 'priority']),
    edits: asRecordOf(field(object, 'edits', path), [...path, 'edits'], (value, at) =>
      asRecordOf(value, at, asString),
    ),
    skip: strings(object, 'skip', path),
    withoutFiles: strings(object, 'withoutFiles', path),
    csvRow: asRecordOf(field(object, 'csvRow', path), [...path, 'csvRow'], (value, at) =>
      asOneOf(['keep', 'ignore'], value, at),
    ),
    problemRows: asOneOf(['leave', 'fix'], field(object, 'problemRows', path), [...path, 'problemRows']),
  };
};

export const parseImportSummary: Parser<ImportSummary> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const progress = asRecord(field(object, 'progress', path), [...path, 'progress']);
  const at = [...path, 'progress'];
  return {
    items: count(object, 'items', path),
    files: count(object, 'files', path),
    thumbnails: count(object, 'thumbnails', path),
    left: count(object, 'left', path),
    failed: count(object, 'failed', path),
    // A summary stored before re-imports skipped existing rows has no count.
    existing: optionalField(object, 'existing', path, asNumber) ?? 0,
    newSets: strings(object, 'newSets', path),
    newTypes: strings(object, 'newTypes', path),
    newFields: strings(object, 'newFields', path),
    itemSetIds: parseListOf((value, at) => brandId<'ItemSetId'>(asIdNumber(value, at)))(
      field(object, 'itemSetIds', path),
      [...path, 'itemSetIds'],
    ),
    existingSets: strings(object, 'existingSets', path),
    progress: { done: count(progress, 'done', at), total: count(progress, 'total', at) },
  };
};

export const parseUndoReport: Parser<UndoReport> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    items: count(object, 'items', path),
    media: count(object, 'media', path),
    itemSets: count(object, 'itemSets', path),
    templates: count(object, 'templates', path),
    fields: count(object, 'fields', path),
    kept: parseListOf((value, at) => {
      const kept = asRecord(value, at);
      return {
        kind: asOneOf(KEPT_KINDS, field(kept, 'kind', at), [...at, 'kind']),
        id: asIdNumber(field(kept, 'id', at), [...at, 'id']),
      };
    })(field(object, 'kept', path), [...path, 'kept']),
  };
};

const recordFrom = (object: Record<string, unknown>, path: Path): ImportRecord => ({
  id: brandId<'ImportId'>(asIdNumber(field(object, 'id', path), [...path, 'id'])),
  folder: text(object, 'folder', path),
  status: asOneOf(STATUSES, field(object, 'status', path), [...path, 'status']),
  undoable: flag(object, 'undoable', path),
  created: text(object, 'created', path),
  finished: nullableField(object, 'finished', path, asString),
  summary: nullableField(object, 'summary', path, parseImportSummary),
});

export const parseImportRecord: Parser<ImportRecord> = (raw, path = []) =>
  recordFrom(asRecord(raw, path), path);

export const parseImportDetail: Parser<ImportDetail> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const jobId = object['jobId'];
  return {
    ...recordFrom(object, path),
    plan: parseImportPlan(field(object, 'plan', path), [...path, 'plan']),
    // The contract's detail is "the import with its plan"; the job id may be absent on a draft.
    jobId: jobId === undefined || jobId === null ? null : asIdNumber(jobId, [...path, 'jobId']),
    undoReport: nullableField(object, 'undoReport', path, parseUndoReport),
    // A server from before job heartbeats does not send it.
    died: optionalField(object, 'died', path, (value, at) => (value === null ? null : asString(value, at))) ?? null,
  };
};

const parseArchiveEntry: Parser<ArchiveEntry> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const files = optionalField(object, 'files', path, asNumber);
  const entry = {
    name: text(object, 'name', path),
    kind: asOneOf(['dir', 'file'], field(object, 'kind', path), [...path, 'kind']),
  };
  return files === undefined ? entry : { ...entry, files };
};

export const parseArchiveListing: Parser<ArchiveListing> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    enabled: flag(object, 'enabled', path),
    path: text(object, 'path', path),
    entries: parseListOf(parseArchiveEntry)(field(object, 'entries', path), [...path, 'entries']),
  };
};

/** `{ jobId }`, the answer of a run and an undo. */
export const parseJobStart: Parser<{ readonly jobId: number }> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return { jobId: asIdNumber(field(object, 'jobId', path), [...path, 'jobId']) };
};

/** Whether a choice is a new type rather than an existing template. */
export const isNewType = (choice: TypeChoice): choice is NewType => typeof choice !== 'number';

/** A route id from the hash, or `null` for one that is not a positive integer. */
export const importIdFromParam = (param: string): ImportId | null => {
  if (!/^\d+$/.test(param)) {
    return null;
  }
  const value = Number.parseInt(param, 10);
  return Number.isSafeInteger(value) && value > 0 ? brandId<'ImportId'>(value) : null;
};

const IMPORTS = '/api/romeka/imports';
const one = (id: ImportId): string => `${IMPORTS}/${String(id)}`;

export const getArchive = async (api: ApiClient, path: string): Promise<ArchiveListing> =>
  parseArchiveListing(await api.requestJson('/api/romeka/archive', new URLSearchParams({ path })));

/** Whether the server has an archive folder to import from, for the nav. */
export const archiveEnabled = async (api: ApiClient): Promise<boolean> =>
  (await getArchive(api, '')).enabled;

export const listImports = async (api: ApiClient): Promise<readonly ImportRecord[]> =>
  parseListOf(parseImportRecord)(await api.requestJson(IMPORTS));

export const createImport = async (api: ApiClient, folder: string): Promise<ImportDetail> =>
  parseImportDetail(await api.writeJson('POST', IMPORTS, { folder }));

export const getImport = async (api: ApiClient, id: ImportId): Promise<ImportDetail> =>
  parseImportDetail(await api.requestJson(one(id)));

export const saveImportPlan = async (
  api: ApiClient,
  id: ImportId,
  plan: ImportPlan,
): Promise<ImportDetail> => parseImportDetail(await api.writeJson('PUT', one(id), { plan }));

/** The route answers 204 with no body. */
export const deleteImport = async (api: ApiClient, id: ImportId): Promise<void> => {
  await api.deleteJson(one(id), () => undefined);
};

/** `GET .../left.csv`: the skipped, left-out and failed rows, each with its reason. */
export const getLeftOutCsv = async (api: ApiClient, id: ImportId): Promise<string> =>
  api.requestText(`${one(id)}/left.csv`);

export const previewImport = async (
  api: ApiClient,
  id: ImportId,
  request: PreviewRequest,
): Promise<Preview> =>
  parsePreview(await api.writeJson('POST', `${one(id)}/preview`, { ...request }));

export const runImport = async (api: ApiClient, id: ImportId): Promise<{ readonly jobId: number }> =>
  parseJobStart(await api.writeJson('POST', `${one(id)}/run`, {}));

export const undoImport = async (api: ApiClient, id: ImportId): Promise<{ readonly jobId: number }> =>
  parseJobStart(await api.writeJson('POST', `${one(id)}/undo`, {}));

/** `GET /api/romeka/jobs/{id}/log`: the job's log so far, as text. */
export const getJobLog = async (api: ApiClient, jobId: number): Promise<string> =>
  api.requestText(`/api/romeka/jobs/${String(jobId)}/log`);
