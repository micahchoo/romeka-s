/**
 * The tus 1.0 core one destination speaks: create, HEAD, PATCH and DELETE an
 * upload under a collection path. An import's staging area and a user's staged
 * uploads share these bytes on the wire, so the protocol lives here once and a
 * target supplies only its collection path and its listing shape.
 *
 * A refusal the admin acts on is thrown as an `UploadRefusedError` whose
 * `refusal` is a union of the codes both destinations share. A code only one
 * destination speaks arrives as `other`, with its wire name and body, so the
 * import's target can name `importFull` and `notDraft` without this module
 * knowing them. Anything else is a plain `ApiError`; a dropped connection is
 * the `TypeError` fetch throws.
 */
import { KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, currentKey } from '../../api/auth';
import { ApiError, defaultBaseUrl } from '../../api/client';
import { asCount, list, member, strict } from '../../api/import-read';
import { ParseError, asString, isRecord, parseApiErrors } from '../../api/parse';
import type { Path } from '../../api/types';
import { localizeServerErrors } from '../../i18n/serverText';

/** One upload the server holds, whole or part way through. */
export interface StoredUpload {
  readonly id: string;
  /** The path relative to what was added, `/`-separated. */
  readonly path: string;
  readonly length: number;
  readonly received: number;
}

/** The largest file and, when the destination has one, the total the server takes. */
export interface UploadLimit {
  readonly file: number;
  readonly total: number | null;
}

/**
 * What a collection read answers. A staging area may state what it holds and
 * its limits; a user's uploads state only the rows, so both are nullable here.
 */
export interface TusListing {
  readonly uploads: readonly StoredUpload[];
  /** The server's own sum when it states one, so a caller can tell a missing member from zero. */
  readonly used: number | null;
  readonly limit: UploadLimit | null;
  /** When a sweep removes the files, or null while they are kept. */
  readonly keepUntil: string | null;
}

export type UploadRefusal =
  | { readonly code: 'uploadPath' }
  | { readonly code: 'fileTooLarge'; readonly limit: number }
  /** An unfinished upload holds the path: resume `upload`. */
  | { readonly code: 'uploadInProgress'; readonly upload: string }
  /** The PATCH offset is not the stored one: HEAD, then continue from there. */
  | { readonly code: 'offsetConflict' }
  /** 403: this account may not add files here. */
  | { readonly code: 'forbidden' }
  /** 404: the upload was removed, or swept unfinished. */
  | { readonly code: 'gone' }
  /** A code one destination speaks: its wire name and the body it came with. */
  | { readonly code: 'other'; readonly wire: string; readonly body: Readonly<Record<string, unknown>> };

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

/** The answer to one PATCH: the stored offset, and the header the caller asked to read. */
export interface TusChunk {
  readonly offset: number;
  readonly header: string | null;
}

export interface AppendOptions {
  readonly signal?: AbortSignal;
  /** A tus extension header this destination reads, such as the import's unzip job. */
  readonly readHeader?: string;
}

export interface TusUploadsOptions<L extends TusListing> {
  /** The collection path, such as `/api/romeka/uploads`. */
  readonly basePath: string;
  readonly baseUrl?: string;
  readonly fetchFn?: typeof fetch;
  /** Reads the collection answer. Each destination states its own shape. */
  readonly readListing: (raw: unknown) => L;
}

const TUS = { 'Tus-Resumable': '1.0.0' } as const;

const asId = (value: unknown, path: Path): string =>
  typeof value === 'number' && Number.isSafeInteger(value) ? String(value) : asString(value, path);

/** One row of a collection answer; `optional` names the members a destination may add. */
export const parseStoredUpload = (raw: unknown, path: Path, optional: readonly string[] = []): StoredUpload => {
  const object = strict(raw, path, ['id', 'path', 'length', 'received'], optional);
  return {
    id: member(object, 'id', path, asId),
    path: member(object, 'path', path, asString),
    length: member(object, 'length', path, asCount),
    received: member(object, 'received', path, asCount),
  };
};

/**
 * The loose collection answer: the rows, then whatever the destination states.
 * A user's staged uploads answer rows only, so a missing total or limit is
 * expected here and reads as null.
 */
export const parseTusListing = (raw: unknown): TusListing => {
  const object = strict(raw, [], ['uploads'], ['used', 'limit', 'keepUntil']);
  const limit = object['limit'];
  const used = object['used'];
  const keepUntil = object['keepUntil'];
  return {
    uploads: member(object, 'uploads', [], list((value, path) => parseStoredUpload(value, path, ['mediaType', 'itemId', 'created', 'expires']))),
    used: used === null || used === undefined ? null : asCount(used, ['used']),
    limit:
      limit === null || limit === undefined
        ? null
        : (() => {
            const objectLimit = strict(limit, ['limit'], ['file', 'total']);
            return {
              file: member(objectLimit, 'file', ['limit'], asCount),
              total: member(objectLimit, 'total', ['limit'], asCount),
            };
          })(),
    keepUntil: keepUntil === null || keepUntil === undefined ? null : asString(keepUntil, ['keepUntil']),
  };
};

/** A response body as JSON, or null for a body that is not JSON. */
export const parseJsonBody = (text: string): unknown => {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return null;
  }
};

/** What a refused answer says, in the active locale and in the server's English. */
export interface RefusedAnswer {
  readonly status: number;
  readonly body: unknown;
  readonly errors: ApiError['errors'];
  readonly source: ApiError['errors'];
  readonly message: string;
}

export const readRefusedAnswer = async (response: Response): Promise<RefusedAnswer> => {
  const text = await response.text().catch(() => '');
  const body = text === '' ? null : parseJsonBody(text);
  const source = body === null ? null : parseApiErrors(body);
  const errors = source === null ? null : await localizeServerErrors(source);
  const message = (errors === null ? undefined : Object.values(errors).flat()[0]) ?? `HTTP ${String(response.status)}`;
  return { status: response.status, body, errors, source, message };
};

/** A refused answer as a plain error, with the server's words kept. */
export const readApiError = async (response: Response): Promise<ApiError> => {
  const answer = await readRefusedAnswer(response);
  return new ApiError(answer.status, answer.errors, answer.message, answer.source, answer.body);
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
    case 'uploadInProgress':
      return { code: 'uploadInProgress', upload: member(at('upload'), 'upload', [], asId) };
    case 'offsetConflict':
      // Members unread on purpose: the client asks HEAD for the stored offset either way.
      return { code: 'offsetConflict' };
    default: {
      const wire = body['code'];
      return typeof wire === 'string' ? { code: 'other', wire, body } : null;
    }
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
export const encodeMetadataValue = (value: string): string => {
  let binary = '';
  for (const byte of new TextEncoder().encode(value)) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
};

/** The `Upload-Metadata` header: a `filename` entry, then the rest. */
export const encodeMetadata = (entries: readonly (readonly [string, string])[]): string =>
  entries.map(([name, value]) => `${name} ${encodeMetadataValue(value)}`).join(',');

export const encodeFilename = (path: string): string => `filename ${encodeMetadataValue(path)}`;

/** A same-origin API URL with the signed-in key attached, or without one when signed out. */
export const keyedUrl = (baseUrl: string, path: string): string => {
  const key = currentKey();
  if (key === null) {
    return `${baseUrl}${path}`;
  }
  const params = new URLSearchParams({ [KEY_IDENTITY_PARAM]: key.identity, [KEY_CREDENTIAL_PARAM]: key.credential });
  return `${baseUrl}${path}?${params.toString()}`;
};

export class TusUploads<L extends TusListing = TusListing> {
  private readonly basePath: string;
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;
  private readonly readListing: (raw: unknown) => L;

  constructor(options: TusUploadsOptions<L>) {
    this.basePath = options.basePath;
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchFn = options.fetchFn ?? ((input, init) => globalThis.fetch(input, init));
    this.readListing = options.readListing;
  }

  async list(params: Readonly<Record<string, string>> = {}): Promise<L> {
    const query = new URLSearchParams(params).toString();
    const path = query === '' ? this.basePath : `${this.basePath}?${query}`;
    const response = await this.send('GET', path, { Accept: 'application/json' });
    return this.readListing(parseJsonBody(await response.text()));
  }

  /** A new upload of `length` bytes at `path`; its id, read from `Location`. */
  async create(path: string, length: number, metadata: readonly (readonly [string, string])[] = []): Promise<string> {
    const response = await this.send('POST', this.basePath, {
      ...TUS,
      'Upload-Length': String(length),
      'Upload-Metadata': encodeMetadata([['filename', path], ...metadata]),
    });
    const location = response.headers.get('Location') ?? '';
    const prefix = `${this.basePath}/`;
    const upload = location.startsWith(prefix) ? location.slice(prefix.length) : '';
    if (upload === '' || upload.includes('/')) {
      throw new ParseError(['Location'], `a path under ${prefix}`);
    }
    return upload;
  }

  /** The stored offset and the declared length. */
  async offset(upload: string): Promise<{ readonly offset: number; readonly length: number }> {
    const response = await this.send('HEAD', this.one(upload), { ...TUS });
    return { offset: headerCount(response, 'Upload-Offset'), length: headerCount(response, 'Upload-Length') };
  }

  async append(upload: string, offset: number, bytes: Blob, options: AppendOptions = {}): Promise<TusChunk> {
    const headers = { ...TUS, 'Content-Type': 'application/offset+octet-stream', 'Upload-Offset': String(offset) };
    const response = await this.send('PATCH', this.one(upload), headers, bytes, options.signal);
    return {
      offset: headerCount(response, 'Upload-Offset'),
      header: options.readHeader === undefined ? null : response.headers.get(options.readHeader),
    };
  }

  async remove(upload: string): Promise<void> {
    await this.send('DELETE', this.one(upload), { ...TUS });
  }

  private one(upload: string): string {
    return `${this.basePath}/${encodeURIComponent(upload)}`;
  }

  private url(path: string): string {
    return keyedUrl(this.baseUrl, path);
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

const refusal = async (response: Response): Promise<ApiError> => {
  const answer = await readRefusedAnswer(response);
  let known: UploadRefusal | null;
  try {
    known = readRefusal(answer.status, answer.body);
  } catch (error) {
    if (!(error instanceof ParseError)) {
      throw error;
    }
    known = null;
  }
  return known === null
    ? new ApiError(answer.status, answer.errors, answer.message, answer.source, answer.body)
    : new UploadRefusedError(known, answer.status, answer.errors, answer.message, answer.source);
};
