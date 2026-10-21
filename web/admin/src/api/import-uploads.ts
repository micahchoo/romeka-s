/**
 * An import's staging area as an upload destination (`docs/import-v2.md`,
 * "Arrival" and "Wire"): tus 1.0 core under
 * `/api/romeka/imports/{id}/uploads`, the `Romeka-Unzip-Job` header a whole zip
 * answers with, and the job routes that unpack polls.
 *
 * The protocol and the refusal union live in `app/upload/tus.ts`; this file
 * names only what an import means: its collection path, its closure rules and
 * the words for its own refusals.
 */
import type { UploadProblem } from '../app/upload/problem';
import type { SentChunk, Unpack, UploadListing, UploadTarget } from '../app/upload/target';
import { TusUploads, keyedUrl, parseJsonBody, parseStoredUpload, readApiError } from '../app/upload/tus';
import type { TusListing } from '../app/upload/tus';
import { defaultBaseUrl } from './client';
import type { ImportId } from './import-plan';
import { asCount, list, member, strict } from './import-read';
import { ParseError, asString, isRecord } from './parse';
import { localizeServerErrors } from '../i18n/serverText';
import { closesImport, importProblemOf } from '../pages/import/upload/problem';

/** The upstream job statuses (`Omeka\Entity\Job`). */
export const JOB_STATUSES = ['starting', 'stopping', 'stopped', 'in_progress', 'completed', 'error'] as const;
export type JobStatus = (typeof JOB_STATUSES)[number];

export interface ImportTargetOptions {
  readonly baseUrl?: string;
  readonly fetchFn?: typeof fetch;
}

/**
 * What the import's collection answers: the rows, the bytes used and the
 * limits. The server always sends all three, so a missing one is a contract
 * break that throws here rather than becoming zero.
 */
export interface ImportListing extends TusListing {
  readonly used: number;
  readonly limit: { readonly file: number; readonly total: number };
}

export const parseImportListing = (raw: unknown): ImportListing => {
  const object = strict(raw, [], ['uploads', 'used', 'limit'], ['keepUntil']);
  const limit = strict(object['limit'], ['limit'], ['file', 'total']);
  const keepUntil = object['keepUntil'];
  return {
    uploads: member(object, 'uploads', [], list(parseStoredUpload)),
    used: member(object, 'used', [], asCount),
    limit: { file: member(limit, 'file', ['limit'], asCount), total: member(limit, 'total', ['limit'], asCount) },
    keepUntil: keepUntil === null || keepUntil === undefined ? null : asString(keepUntil, ['keepUntil']),
  };
};

/** The job routes an import's uploads report through. */
export class ImportJobs {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(options: ImportTargetOptions = {}) {
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchFn = options.fetchFn ?? ((input, init) => globalThis.fetch(input, init));
  }

  async jobStatus(jobId: number): Promise<JobStatus> {
    const body = parseJsonBody(await (await this.get(`/api/jobs/${String(jobId)}`, 'application/json')).text());
    const status = isRecord(body) ? JOB_STATUSES.find((candidate) => candidate === body['o:status']) : undefined;
    if (status === undefined) {
      throw new ParseError(['o:status'], `one of ${JOB_STATUSES.join(', ')}`);
    }
    return status;
  }

  /** The last line of a job's log, in the active locale: the reason a job ended in error. */
  async jobReason(jobId: number): Promise<string> {
    const lines = (await (await this.get(`/api/romeka/jobs/${String(jobId)}/log`, 'text/plain')).text())
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line !== '');
    const last = lines.at(-1) ?? '';
    const translated = await localizeServerErrors({ error: [last] });
    return translated['error']?.[0] ?? last;
  }

  private async get(path: string, accept: string): Promise<Response> {
    const response = await this.fetchFn(keyedUrl(this.baseUrl, path), {
      headers: { Accept: accept },
      credentials: 'same-origin',
    });
    if (!response.ok) {
      // The job routes answer the same error document as the rest of `/api`.
      throw await readApiError(response);
    }
    return response;
  }
}

/** The application-wide import job client. */
export const importJobs = new ImportJobs();

/** One import's staging area. */
export class ImportUploads implements UploadTarget {
  private readonly tus: TusUploads<ImportListing>;
  private readonly jobs: ImportJobs;

  constructor(importId: ImportId, options: ImportTargetOptions = {}, jobs?: ImportJobs) {
    this.tus = new TusUploads({
      basePath: `/api/romeka/imports/${String(importId)}/uploads`,
      readListing: parseImportListing,
      ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
      ...(options.fetchFn === undefined ? {} : { fetchFn: options.fetchFn }),
    });
    this.jobs = jobs ?? new ImportJobs(options);
  }

  async list(): Promise<UploadListing> {
    const listing = await this.tus.list();
    return { uploads: listing.uploads, used: listing.used, limit: listing.limit };
  }

  create(path: string, length: number): Promise<string> {
    return this.tus.create(path, length);
  }

  offset(upload: string): Promise<{ readonly offset: number; readonly length: number }> {
    return this.tus.offset(upload);
  }

  async append(upload: string, offset: number, bytes: Blob, signal?: AbortSignal): Promise<SentChunk> {
    const chunk = await this.tus.append(upload, offset, bytes, {
      ...(signal === undefined ? {} : { signal }),
      readHeader: 'Romeka-Unzip-Job',
    });
    const job = chunk.header === null ? null : Number(chunk.header);
    return {
      offset: chunk.offset,
      unpack: job === null || !Number.isSafeInteger(job) ? null : this.unpack(job),
    };
  }

  remove(upload: string): Promise<void> {
    return this.tus.remove(upload);
  }

  problemOf(error: unknown): UploadProblem {
    return importProblemOf(error);
  }

  closes(problem: UploadProblem): boolean {
    return closesImport(problem);
  }

  private unpack(job: number): Unpack {
    return {
      check: async () => {
        const status = await this.jobs.jobStatus(job);
        if (status === 'completed') {
          return { kind: 'done' };
        }
        if (status === 'error' || status === 'stopped') {
          const reason = await this.jobs.jobReason(job);
          return { kind: 'failed', problem: { kind: 'zipRefused', reason } };
        }
        return { kind: 'running' };
      },
    };
  }
}

/** One import's staging area, ready to hand to `UploadSession`. */
export const importTarget = (importId: ImportId, options: ImportTargetOptions = {}): UploadTarget =>
  new ImportUploads(importId, options);
