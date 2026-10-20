/**
 * The upload routes of an import (`docs/import-v2.md`, "Arrival" and "Wire"):
 * tus 1.0 core under `/api/romeka/imports/{id}/uploads`, plus the JSON listing
 * and the job routes an unpacked zip reports through.
 *
 * A refusal the admin acts on is thrown as an `UploadRefusedError` whose
 * `refusal` is a closed union, so the caller switches over it instead of
 * reading status codes. Anything else is a plain `ApiError`; a dropped
 * connection is the `TypeError` fetch throws.
 */
import { KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, currentKey } from './auth';
import { ApiError, defaultBaseUrl } from './client';
import type { ImportId } from './import-plan';
import { asCount, list, member, strict } from './import-read';
import { ParseError, asString, isRecord, parseApiErrors } from './parse';
import type { Path } from './types';
import { localizeServerErrors } from '../i18n/serverText';

export interface StoredUpload {
  readonly id: string;
  /** The path relative to what was added, `/`-separated. */
  readonly path: string;
  readonly length: number;
  readonly received: number;
}

export interface UploadListing {
  readonly uploads: readonly StoredUpload[];
  /** Every upload's length, unfinished ones too. */
  readonly used: number;
  readonly limit: { readonly file: number; readonly total: number };
  /** When the sweep removes a draft's files (ISO 8601), or null while they are kept. */
  readonly keepUntil: string | null;
}

export type UploadRefusal =
  | { readonly code: 'uploadPath' }
  | { readonly code: 'fileTooLarge'; readonly limit: number }
  | { readonly code: 'importFull'; readonly limit: number; readonly used: number }
  /** An unfinished upload holds the path: resume `upload`. */
  | { readonly code: 'uploadInProgress'; readonly upload: string }
  | { readonly code: 'notDraft' }
  /** The PATCH offset is not the stored one: HEAD, then continue from there. */
  | { readonly code: 'offsetConflict' }
  /** 403: only the import's owner and global admins may add files. */
  | { readonly code: 'forbidden' }
  /** 404: the upload was removed, or swept after 24 hours unfinished. */
  | { readonly code: 'gone' };

export class UploadRefusedError extends ApiError {
  readonly refusal: UploadRefusal;

  constructor(refusal: UploadRefusal, status: number, errors: ApiError['errors'], message: string, source: ApiError['errors']) {
    super(status, errors, message, source);
    this.name = 'UploadRefusedError';
    this.refusal = refusal;
  }
}

export const refusalOf = (error: unknown): UploadRefusal | null =>
  error instanceof UploadRefusedError ? error.refusal : null;

/** The upstream job statuses (`Omeka\Entity\Job`). */
export const JOB_STATUSES = ['starting', 'stopping', 'stopped', 'in_progress', 'completed', 'error'] as const;
export type JobStatus = (typeof JOB_STATUSES)[number];

export interface SentChunk {
  readonly offset: number;
  /** The job that unpacks a `.zip` whose last byte this PATCH carried. */
  readonly unzipJob: number | null;
}

export interface UploadApiOptions {
  readonly baseUrl?: string;
  readonly fetchFn?: typeof fetch;
}

const TUS = { 'Tus-Resumable': '1.0.0' } as const;

const asId = (value: unknown, path: Path): string =>
  typeof value === 'number' && Number.isSafeInteger(value) ? String(value) : asString(value, path);

const parseStored = (raw: unknown, path: Path): StoredUpload => {
  const object = strict(raw, path, ['id', 'path', 'length', 'received']);
  return {
    id: member(object, 'id', path, asId),
    path: member(object, 'path', path, asString),
    length: member(object, 'length', path, asCount),
    received: member(object, 'received', path, asCount),
  };
};

export const parseListing = (raw: unknown): UploadListing => {
  const object = strict(raw, [], ['uploads', 'used', 'limit'], ['keepUntil']);
  const keepUntil = object['keepUntil'];
  const limit = strict(object['limit'], ['limit'], ['file', 'total']);
  return {
    uploads: member(object, 'uploads', [], list(parseStored)),
    used: member(object, 'used', [], asCount),
    limit: { file: member(limit, 'file', ['limit'], asCount), total: member(limit, 'total', ['limit'], asCount) },
    keepUntil: keepUntil === null || keepUntil === undefined ? null : asString(keepUntil, ['keepUntil']),
  };
};

const BASE = ['errors', 'code'];

const readRefusal = (status: number, body: unknown): UploadRefusal | null => {
  if (status === 403) {
    return { code: 'forbidden' };
  }
  if (status === 404) {
    return { code: 'gone' };
  }
  if (!isRecord(body)) {
    return null;
  }
  const at = (...keys: string[]): Record<string, unknown> => strict(body, [], [...BASE, ...keys]);
  switch (body['code']) {
    case 'uploadPath':
      at();
      return { code: 'uploadPath' };
    case 'fileTooLarge':
      return { code: 'fileTooLarge', limit: member(at('limit'), 'limit', [], asCount) };
    case 'importFull': {
      const object = at('limit', 'used');
      return { code: 'importFull', limit: member(object, 'limit', [], asCount), used: member(object, 'used', [], asCount) };
    }
    case 'uploadInProgress':
      return { code: 'uploadInProgress', upload: member(at('upload'), 'upload', [], asId) };
    case 'notDraft':
      // The import routes' notDraft also names the status; the admin needs only the code.
      strict(body, [], BASE, ['status']);
      return { code: 'notDraft' };
    case 'offsetConflict':
      // Members unread on purpose: the client asks HEAD for the stored offset either way.
      return { code: 'offsetConflict' };
    default:
      return null;
  }
};

const parseJson = (text: string): unknown => {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return null;
  }
};

const headerCount = (response: Response, name: string): number => {
  const value = Number(response.headers.get(name));
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new ParseError([name], 'a whole number from 0');
  }
  return value;
};

/** The `Upload-Metadata` value: base64 of the UTF-8 bytes, which `btoa` cannot take directly. */
export const encodeFilename = (path: string): string => {
  let binary = '';
  for (const byte of new TextEncoder().encode(path)) {
    binary += String.fromCharCode(byte);
  }
  return `filename ${btoa(binary)}`;
};

export class UploadApi {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(options: UploadApiOptions = {}) {
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchFn = options.fetchFn ?? ((input, init) => globalThis.fetch(input, init));
  }

  async list(id: ImportId): Promise<UploadListing> {
    const response = await this.send('GET', uploads(id), { Accept: 'application/json' });
    return parseListing(parseJson(await response.text()));
  }

  /** A new upload of `length` bytes at `path`; its id, read from `Location`. */
  async create(id: ImportId, path: string, length: number): Promise<string> {
    const response = await this.send('POST', uploads(id), {
      ...TUS,
      'Upload-Length': String(length),
      'Upload-Metadata': encodeFilename(path),
    });
    const location = response.headers.get('Location') ?? '';
    const prefix = `${uploads(id)}/`;
    const upload = location.startsWith(prefix) ? location.slice(prefix.length) : '';
    if (upload === '' || upload.includes('/')) {
      throw new ParseError(['Location'], `a path under ${prefix}`);
    }
    return upload;
  }

  /** The stored offset and the declared length. */
  async offset(id: ImportId, upload: string): Promise<{ readonly offset: number; readonly length: number }> {
    const response = await this.send('HEAD', one(id, upload), { ...TUS });
    return { offset: headerCount(response, 'Upload-Offset'), length: headerCount(response, 'Upload-Length') };
  }

  async append(id: ImportId, upload: string, offset: number, bytes: Blob, signal?: AbortSignal): Promise<SentChunk> {
    const headers = { ...TUS, 'Content-Type': 'application/offset+octet-stream', 'Upload-Offset': String(offset) };
    const response = await this.send('PATCH', one(id, upload), headers, bytes, signal);
    const job = response.headers.get('Romeka-Unzip-Job');
    return { offset: headerCount(response, 'Upload-Offset'), unzipJob: job === null ? null : Number(job) };
  }

  async remove(id: ImportId, upload: string): Promise<void> {
    await this.send('DELETE', one(id, upload), { ...TUS });
  }

  async jobStatus(jobId: number): Promise<JobStatus> {
    const response = await this.send('GET', `/api/jobs/${String(jobId)}`, { Accept: 'application/json' });
    const body = parseJson(await response.text());
    const status = isRecord(body) ? JOB_STATUSES.find((candidate) => candidate === body['o:status']) : undefined;
    if (status === undefined) {
      throw new ParseError(['o:status'], `one of ${JOB_STATUSES.join(', ')}`);
    }
    return status;
  }

  /** The last line of a job's log, in the active locale: the reason a job ended in error. */
  async jobReason(jobId: number): Promise<string> {
    const response = await this.send('GET', `/api/romeka/jobs/${String(jobId)}/log`, { Accept: 'text/plain' });
    const lines = (await response.text()).split('\n').map((line) => line.trim()).filter((line) => line !== '');
    const last = lines.at(-1) ?? '';
    const translated = await localizeServerErrors({ error: [last] });
    return translated['error']?.[0] ?? last;
  }

  private url(path: string): string {
    const key = currentKey();
    if (key === null) {
      return `${this.baseUrl}${path}`;
    }
    const params = new URLSearchParams({ [KEY_IDENTITY_PARAM]: key.identity, [KEY_CREDENTIAL_PARAM]: key.credential });
    return `${this.baseUrl}${path}?${params.toString()}`;
  }

  private async send(
    method: string,
    path: string,
    headers: Record<string, string>,
    body?: Blob,
    signal?: AbortSignal,
  ): Promise<Response> {
    const response = await this.fetchFn(this.url(path), {
      method,
      headers,
      credentials: 'same-origin',
      ...(body === undefined ? {} : { body }),
      ...(signal === undefined ? {} : { signal }),
    });
    if (!response.ok) {
      throw await refusal(response);
    }
    return response;
  }
}

const uploads = (id: ImportId): string => `/api/romeka/imports/${String(id)}/uploads`;
const one = (id: ImportId, upload: string): string => `${uploads(id)}/${encodeURIComponent(upload)}`;

const refusal = async (response: Response): Promise<ApiError> => {
  const text = await response.text().catch(() => '');
  const body = text === '' ? null : parseJson(text);
  const source = body === null ? null : parseApiErrors(body);
  const errors = source === null ? null : await localizeServerErrors(source);
  const message = (errors === null ? undefined : Object.values(errors).flat()[0]) ?? `HTTP ${String(response.status)}`;
  let known: UploadRefusal | null;
  try {
    known = readRefusal(response.status, body);
  } catch (error) {
    if (!(error instanceof ParseError)) {
      throw error;
    }
    known = null;
  }
  return known === null
    ? new ApiError(response.status, errors, message, source)
    : new UploadRefusedError(known, response.status, errors, message, source);
};

/** The application-wide upload client. */
export const uploadApi = new UploadApi();
