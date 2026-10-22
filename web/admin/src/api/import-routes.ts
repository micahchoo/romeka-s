/**
 * The import routes (`docs/import-v2.md`, "Wire"): one typed call per route,
 * each answer parsed at the boundary.
 *
 * This is a client of its own rather than a method set on `ApiClient`. A 409
 * carries `code` and members besides the error document, and `ApiClient`
 * keeps only the document, so the two cannot share a transport until
 * `ApiError` keeps the parsed body. The page chunk that reads imports is the
 * only one that loads this file.
 */
import { KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, currentKey } from './auth';
import { ApiError, defaultBaseUrl } from './client';
import { ImportConflictError, SeriesRefusalError, parseConflict, parseSeriesRefusal } from './import-conflict';
import type { ImportId, ImportPlan, LevelId, SourcePlan } from './import-plan';
import { parseArchiveListing, parseHistory, parseImportDetail, parseJobStart, parsePairsAdded, parseScanResult } from './import-view';
import type { ArchiveListing, History, ImportDetail, JobStart, PairsAdded, ScanResult } from './import-view';
import { parseUndoPreview } from './import-undo';
import type { ResourceKind, UndoPreview } from './import-undo';
import { hasSections, parsePreview } from './import-sections';
import type { PreviewOptions } from './import-queries';
import type { PreviewWith, Section } from './import-sections';
import { parseApiErrors, ParseError } from './parse';
import { parseStarters } from './starters';
import type { StarterTable } from './starters';
import { localizeServerErrors } from '../i18n/serverText';
import type { ApiErrors } from './types';

export type CreateImport = { readonly sources: readonly SourcePlan[] } | { readonly copyFrom: ImportId };

/** The resources an undo restores or deletes although something else touched them. */
export type ForcedKind = ResourceKind;
export type Force = 'all' | readonly { readonly kind: ForcedKind; readonly id: number }[];

export interface RunRequest {
  /** The digest the Check section showed. */
  readonly digest: string;
  /** For a delete: the number of resources it deletes. */
  readonly confirm?: number;
}

export interface ImportApiOptions {
  readonly baseUrl?: string;
  readonly fetchFn?: typeof fetch;
}

const IMPORTS = '/api/romeka/imports';
const one = (id: ImportId): string => `${IMPORTS}/${String(id)}`;

const seriesParams = (series: LevelId | undefined): URLSearchParams =>
  new URLSearchParams(series === undefined ? {} : { series });

const templateParams = (columns: readonly string[], series: LevelId | undefined): URLSearchParams =>
  new URLSearchParams({ columns: columns.join(','), ...(series === undefined ? {} : { series }) });

const firstMessage = (errors: ApiErrors | null): string | null => {
  const messages = errors === null ? [] : [...(errors['error'] ?? []), ...Object.values(errors).flat()];
  return messages[0] ?? null;
};

const parseJson = (text: string): unknown => {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return null;
  }
};

export class ImportApi {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(options: ImportApiOptions = {}) {
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchFn = options.fetchFn ?? ((input, init) => globalThis.fetch(input, init));
  }

  async archive(query: { readonly root?: string; readonly path?: string } = {}): Promise<ArchiveListing> {
    const params = new URLSearchParams();
    if (query.root !== undefined) {
      params.set('root', query.root);
    }
    if (query.path !== undefined) {
      params.set('path', query.path);
    }
    return parseArchiveListing(await this.json('GET', '/api/romeka/archive', { params }));
  }

  async starters(): Promise<StarterTable> {
    return parseStarters(await this.json('GET', '/api/romeka/starters'));
  }

  async history(query: { readonly page?: number; readonly perPage?: number } = {}): Promise<History> {
    const params = new URLSearchParams();
    if (query.page !== undefined) {
      params.set('page', String(query.page));
    }
    if (query.perPage !== undefined) {
      params.set('perPage', String(query.perPage));
    }
    return parseHistory(await this.json('GET', IMPORTS, { params }));
  }

  /** A draft with a default plan, over new sources or a copy of an import's plan. */
  async create(request: CreateImport): Promise<ImportDetail> {
    return parseImportDetail(await this.json('POST', IMPORTS, { body: request }));
  }

  async get(id: ImportId): Promise<ImportDetail> {
    return parseImportDetail(await this.json('GET', one(id)));
  }

  /** 409 `stale` when `revision` is not the stored one, 409 `notDraft` once the import has started. */
  async save(id: ImportId, plan: ImportPlan, revision: number, options: { readonly keepalive?: boolean } = {}): Promise<ImportDetail> {
    return parseImportDetail(await this.json('PUT', one(id), { body: { plan, revision }, ...options }));
  }

  /** A draft only; the route answers 204. */
  async remove(id: ImportId): Promise<void> {
    await this.json('DELETE', one(id));
  }

  async scan(id: ImportId): Promise<ScanResult> {
    return parseScanResult(await this.json('POST', `${one(id)}/scan`));
  }

  /**
   * The sections named in `include`, derived from the stored plan or from
   * `options.plan`. The answer holds exactly those sections.
   */
  async preview<S extends Section>(
    id: ImportId,
    include: readonly S[],
    options: PreviewOptions = {},
  ): Promise<PreviewWith<S>> {
    const raw = await this.json('POST', `${one(id)}/preview`, { body: { include, ...options } });
    const preview = parsePreview(include)(raw);
    if (!hasSections(preview, include)) {
      throw new ParseError([], 'every section the request named');
    }
    return preview;
  }

  /** 409 `changed`, `blocked`, `nothing`, `confirm`, `busy` or `cannotRun`. A retry of a failed run is the same call. */
  async run(id: ImportId, request: RunRequest): Promise<JobStart> {
    return parseJobStart(await this.json('POST', `${one(id)}/run`, { body: request }));
  }

  /** The job stops after the record in hand; 409 `notRunning` for an import that does not run. */
  async stop(id: ImportId): Promise<void> {
    await this.json('POST', `${one(id)}/stop`);
  }

  /** What an undo with nothing forced would do now, and what it would keep and why; it writes nothing. 409 `cannotUndo`. */
  async undoPreview(id: ImportId): Promise<UndoPreview> {
    return parseUndoPreview(await this.json('GET', `${one(id)}/undo/preview`));
  }

  async undo(id: ImportId, force?: Force): Promise<JobStart> {
    const body = force === undefined ? {} : { force };
    return parseJobStart(await this.json('POST', `${one(id)}/undo`, { body }));
  }

  /** The left-out and failed entries of a run that ended, as the CSV the server stored. */
  async leftCsv(id: ImportId): Promise<string> {
    return this.text('GET', `${one(id)}/left.csv`);
  }

  /**
   * The list to fill in, one row per item record of `series`, with a column
   * per term in `columns`. A plan of several series needs `series`, else
   * the server refuses it (`seriesRefusalOf`).
   */
  async templateCsv(id: ImportId, columns: readonly string[], series?: LevelId): Promise<string> {
    return this.text('GET', `${one(id)}/template.csv`, templateParams(columns, series));
  }

  /** The address of the downloaded list, for a link: the browser saves the bytes as the server wrote them. */
  templateUrl(id: ImportId, columns: readonly string[], series?: LevelId): string {
    return this.url(`${one(id)}/template.csv`, templateParams(columns, series));
  }

  /** A filled list (`.csv` or `.tsv`, at most 50 MB) added to `series` as a sheet; a plan of several series needs `series`. */
  async addList(id: ImportId, file: File, series?: LevelId): Promise<ImportDetail> {
    const form = new FormData();
    form.append('file', file, file.name);
    return parseImportDetail(await this.json('POST', `${one(id)}/sheets`, { form, params: seriesParams(series) }));
  }

  /** The address of the pairs list, for a link: one line for each list row that names no file, with an empty Files column to fill. */
  pairsUrl(id: ImportId): string {
    return this.url(`${one(id)}/pairs.csv`);
  }

  /** A filled pairs list (`.csv`, `.tsv` or a workbook, at most 50 MB): one `pair` decision for each Files cell that names exactly one file. */
  async addPairs(id: ImportId, file: File): Promise<PairsAdded> {
    const form = new FormData();
    form.append('file', file, file.name);
    return parsePairsAdded(await this.json('POST', `${one(id)}/pairs`, { form }));
  }

  async jobLog(jobId: number): Promise<string> {
    return this.text('GET', `/api/romeka/jobs/${String(jobId)}/log`);
  }

  private url(path: string, params: URLSearchParams = new URLSearchParams()): string {
    const key = currentKey();
    if (key !== null) {
      params.set(KEY_IDENTITY_PARAM, key.identity);
      params.set(KEY_CREDENTIAL_PARAM, key.credential);
    }
    const query = params.toString();
    return query === '' ? `${this.baseUrl}${path}` : `${this.baseUrl}${path}?${query}`;
  }

  private async send(
    method: string,
    path: string,
    options: {
      readonly params?: URLSearchParams;
      readonly body?: unknown;
      readonly form?: FormData;
      /** Lets the request outlive the page, for a save sent as the page closes. */
      readonly keepalive?: boolean;
    } = {},
  ): Promise<Response> {
    const headers: Record<string, string> = { Accept: 'application/json' };
    let body: string | FormData | undefined;
    if (options.form !== undefined) {
      body = options.form;
    } else if (options.body !== undefined || method === 'POST' || method === 'PUT') {
      headers['Content-Type'] = 'application/json';
      body = JSON.stringify(options.body ?? {});
    }
    const response = await this.fetchFn(this.url(path, options.params), {
      method,
      headers,
      credentials: 'same-origin',
      ...(options.keepalive === true ? { keepalive: true } : {}),
      ...(body === undefined ? {} : { body }),
    });
    if (!response.ok) {
      throw await this.refusal(response);
    }
    return response;
  }

  private async json(
    method: string,
    path: string,
    options: {
      readonly params?: URLSearchParams;
      readonly body?: unknown;
      readonly form?: FormData;
      /** Lets the request outlive the page, for a save sent as the page closes. */
      readonly keepalive?: boolean;
    } = {},
  ): Promise<unknown> {
    const text = await (await this.send(method, path, options)).text();
    return text === '' ? null : parseJson(text);
  }

  /** The body as UTF-8 with a byte order mark kept: `Response.text()` drops it, and the downloaded list needs it for Excel. */
  private async text(method: string, path: string, params?: URLSearchParams): Promise<string> {
    const response = await this.send(method, path, params === undefined ? {} : { params });
    return new TextDecoder('utf-8', { ignoreBOM: true }).decode(await response.arrayBuffer());
  }

  private async refusal(response: Response): Promise<ApiError> {
    const text = await response.text().catch(() => '');
    const body = text === '' ? null : parseJson(text);
    const source = body === null ? null : parseApiErrors(body);
    const errors = source === null ? null : await localizeServerErrors(source);
    const message = firstMessage(errors) ?? `HTTP ${String(response.status)}`;
    const conflict = response.status === 409 ? parseConflict(body) : null;
    if (conflict !== null) {
      return new ImportConflictError(conflict, errors, message, source);
    }
    const refusal = response.status === 400 ? parseSeriesRefusal(body) : null;
    return refusal === null ? new ApiError(response.status, errors, message, source) : new SeriesRefusalError(refusal, errors, message, source);
  }
}

/** The application-wide import client. Tests stub `globalThis.fetch` before use. */
export const importApi = new ImportApi();
