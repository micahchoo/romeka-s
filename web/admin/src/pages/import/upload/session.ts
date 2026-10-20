/**
 * The files a person adds to one import from this computer, and the queue
 * that sends them: a few at a time, in chunks, resumable from the offset the
 * server holds.
 *
 * Each file is a row whose `phase` is the one thing it is doing. The bytes,
 * the server's upload id and the abort handle stay out of the snapshot, so a
 * view renders rows without reaching into the transfer. No React here: the
 * view subscribes (`useSyncExternalStore`), and a test drives it directly.
 */
import type { ImportId } from '../../../api/import-plan';
import { refusalOf } from '../../../api/import-uploads';
import type { StoredUpload, UploadApi } from '../../../api/import-uploads';
import type { Picked } from './pick';
import { closesImport, problemOf } from './problem';
import type { UploadProblem } from './problem';

export type Phase =
  | { readonly kind: 'waiting' }
  | { readonly kind: 'sending' }
  | { readonly kind: 'paused' }
  /** Unfinished on the server from an earlier visit; this page does not hold its bytes. */
  | { readonly kind: 'needsFile' }
  /** A zip that arrived whole and is being unpacked on the server. */
  | { readonly kind: 'unpacking' }
  | { readonly kind: 'done'; readonly unpacked: boolean }
  | { readonly kind: 'failed'; readonly problem: UploadProblem };

export interface UploadRow {
  /** The path, unique among the rows. */
  readonly key: string;
  readonly path: string;
  readonly size: number;
  /** Bytes the server holds. */
  readonly sent: number;
  readonly phase: Phase;
}

export interface UploadSnapshot {
  readonly loaded: boolean;
  readonly rows: readonly UploadRow[];
  /** Files on the server whole: those from earlier visits and the rows done here. */
  readonly stored: { readonly count: number; readonly bytes: number };
  readonly limit: { readonly file: number; readonly total: number } | null;
  readonly used: number;
  /** Dot files and dot folders left out of what was added. */
  readonly skipped: number;
  /** Files added again whose path and size a stored file already has. */
  readonly alreadyHere: number;
  /** Set when no file can be added to this import any more. */
  readonly closed: UploadProblem | null;
  readonly loadProblem: UploadProblem | null;
}

export interface UploadSessionOptions {
  readonly api: UploadApi;
  readonly importId: ImportId;
  /** Files sent at once. */
  readonly concurrency?: number;
  readonly chunkSize?: number;
  /** Tries after a dropped connection before the file fails. */
  readonly retries?: number;
  readonly wait?: (ms: number) => Promise<void>;
  /** The files on the server changed: one arrived whole, a zip was unpacked, or a stored one was removed. */
  readonly onFilesChanged?: () => void;
}

interface Held {
  file: File | null;
  upload: string | null;
  controller: AbortController | null;
}

const MIB = 1024 * 1024;
const POLL_MS = 1000;
const ACTIVE = new Set<Phase['kind']>(['waiting', 'sending', 'unpacking']);

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, ms);
  });

export class UploadSession {
  private readonly api: UploadApi;
  private readonly importId: ImportId;
  private readonly concurrency: number;
  private readonly chunkSize: number;
  private readonly retries: number;
  private readonly wait: (ms: number) => Promise<void>;
  private readonly onFilesChanged: () => void;
  private readonly held = new Map<string, Held>();
  /** Whole files on the server that no row stands for, by path. */
  private earlier = new Map<string, StoredUpload>();
  private listeners = new Set<() => void>();
  private loading: Promise<void> | null = null;
  private disposed = false;
  private state: UploadSnapshot = {
    loaded: false,
    rows: [],
    stored: { count: 0, bytes: 0 },
    limit: null,
    used: 0,
    skipped: 0,
    alreadyHere: 0,
    closed: null,
    loadProblem: null,
  };

  constructor(options: UploadSessionOptions) {
    this.api = options.api;
    this.importId = options.importId;
    this.concurrency = options.concurrency ?? 3;
    this.chunkSize = options.chunkSize ?? 8 * MIB;
    this.retries = options.retries ?? 4;
    this.wait = options.wait ?? sleep;
    this.onFilesChanged = options.onFilesChanged ?? (() => undefined);
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  readonly snapshot = (): UploadSnapshot => this.state;

  /** Reads what the server already holds: limits, whole files, and unfinished ones to resume. */
  load(): Promise<void> {
    this.loading ??= this.read().finally(() => {
      this.loading = null;
    });
    return this.loading;
  }

  async add(picked: Picked): Promise<void> {
    if (!this.state.loaded) {
      await this.load();
    }
    let alreadyHere = this.state.alreadyHere;
    let rows = this.state.rows;
    for (const { path, file } of picked.files) {
      const row = rows.find((candidate) => candidate.path === path);
      const stored = this.earlier.get(path);
      if (row === undefined && stored?.length === file.size) {
        alreadyHere += 1;
        continue;
      }
      if (row !== undefined && row.phase.kind !== 'needsFile' && row.phase.kind !== 'failed') {
        alreadyHere += 1;
        continue;
      }
      const held = this.held.get(path) ?? { file: null, upload: null, controller: null };
      held.file = file;
      this.held.set(path, held);
      const next: UploadRow = { key: path, path, size: file.size, sent: row?.size === file.size ? row.sent : 0, phase: this.firstPhase(file.size) };
      rows = row === undefined ? [...rows, next] : rows.map((candidate) => (candidate === row ? next : candidate));
    }
    this.set({ rows, alreadyHere, skipped: this.state.skipped + picked.skipped });
    this.pump();
  }

  pause(key: string): void {
    this.patch(key, (row) => {
      if (row.phase.kind !== 'waiting' && row.phase.kind !== 'sending') {
        return row;
      }
      // The chunk in flight is lost; the server keeps whole chunks only.
      this.held.get(key)?.controller?.abort();
      return { ...row, phase: { kind: 'paused' } };
    });
  }

  resume(key: string): void {
    this.patch(key, (row) => (row.phase.kind === 'paused' ? { ...row, phase: { kind: 'waiting' } } : row));
    this.pump();
  }

  pauseAll(): void {
    for (const row of this.state.rows) {
      this.pause(row.key);
    }
  }

  resumeAll(): void {
    this.patchAll((row) => (row.phase.kind === 'paused' ? { ...row, phase: { kind: 'waiting' } } : row));
    this.pump();
  }

  retry(key: string): void {
    if (this.state.closed !== null) {
      return;
    }
    this.patch(key, (row) => {
      if (row.phase.kind !== 'failed') {
        return row;
      }
      if (row.phase.problem.kind === 'gone') {
        this.forget(key);
        return { ...row, sent: 0, phase: { kind: 'waiting' } };
      }
      return { ...row, phase: { kind: 'waiting' } };
    });
    this.pump();
  }

  /** Removes the file from the server, finished or not, and its row. */
  async remove(key: string): Promise<void> {
    const index = this.state.rows.findIndex((row) => row.key === key);
    const row = this.state.rows[index];
    const held = this.held.get(key);
    if (row === undefined) {
      return;
    }
    held?.controller?.abort();
    this.held.delete(key);
    this.set({ rows: this.state.rows.filter((candidate) => candidate !== row) });
    const upload = held?.upload ?? null;
    if (upload === null || (row.phase.kind === 'done' && row.phase.unpacked)) {
      return;
    }
    try {
      await this.api.remove(this.importId, upload);
    } catch (error) {
      if (refusalOf(error)?.code !== 'gone') {
        const rows = [...this.state.rows];
        rows.splice(index, 0, { ...row, phase: { kind: 'failed', problem: problemOf(error) } });
        this.held.set(key, { file: held?.file ?? null, upload, controller: null });
        this.set({ rows });
        return;
      }
    }
    this.set({ used: Math.max(0, this.state.used - row.size) });
    if (row.phase.kind === 'done') {
      this.onFilesChanged();
    }
  }

  /** Resolves once no row is waiting, sending or unpacking. */
  whenIdle(): Promise<void> {
    return new Promise((resolve) => {
      const check = (): void => {
        if (!this.state.rows.some((row) => ACTIVE.has(row.phase.kind))) {
          stop();
          resolve();
        }
      };
      const stop = this.subscribe(check);
      check();
    });
  }

  /** Stops every transfer; the server keeps what it holds, and a later visit resumes it. */
  dispose(): void {
    this.disposed = true;
    for (const held of this.held.values()) {
      held.controller?.abort();
    }
    this.listeners.clear();
  }

  private async read(): Promise<void> {
    try {
      const listing = await this.api.list(this.importId);
      const known = new Set([...this.held.values()].map((held) => held.upload));
      const rows = [...this.state.rows];
      this.earlier = new Map();
      for (const stored of listing.uploads) {
        if (known.has(stored.id)) {
          continue;
        }
        if (stored.received >= stored.length) {
          this.earlier.set(stored.path, stored);
        } else if (!rows.some((row) => row.path === stored.path)) {
          this.held.set(stored.path, { file: null, upload: stored.id, controller: null });
          rows.push({ key: stored.path, path: stored.path, size: stored.length, sent: stored.received, phase: { kind: 'needsFile' } });
        }
      }
      this.set({ loaded: true, rows, limit: listing.limit, used: listing.used, loadProblem: null });
    } catch (error) {
      const problem = problemOf(error);
      this.set({ loaded: true, loadProblem: problem, closed: closesImport(problem) ? problem : this.state.closed });
    }
  }

  private firstPhase(size: number): Phase {
    const closed = this.state.closed;
    if (closed !== null) {
      return { kind: 'failed', problem: closed };
    }
    const limit = this.state.limit;
    return limit !== null && size > limit.file
      ? { kind: 'failed', problem: { kind: 'fileTooLarge', limit: limit.file } }
      : { kind: 'waiting' };
  }

  private pump(): void {
    if (this.disposed) {
      return;
    }
    let free = this.concurrency - this.state.rows.filter((row) => row.phase.kind === 'sending').length;
    for (const row of this.state.rows) {
      if (free <= 0) {
        return;
      }
      if (row.phase.kind === 'waiting') {
        free -= 1;
        this.start(row.key);
      }
    }
  }

  private start(key: string): void {
    const held = this.held.get(key);
    if (held === undefined) {
      return;
    }
    const controller = new AbortController();
    held.controller = controller;
    this.patch(key, (row) => ({ ...row, phase: { kind: 'sending' } }));
    void this.run(key, held, controller);
  }

  private async run(key: string, held: Held, controller: AbortController): Promise<void> {
    const live = (): boolean => !this.disposed && !controller.signal.aborted && this.held.get(key) === held;
    try {
      const job = await this.transfer(key, held, controller.signal);
      if (!live()) {
        return;
      }
      if (job === null) {
        this.patch(key, (row) => ({ ...row, sent: row.size, phase: { kind: 'done', unpacked: false } }));
        this.onFilesChanged();
        return;
      }
      this.patch(key, (row) => ({ ...row, sent: row.size, phase: { kind: 'unpacking' } }));
      this.pump();
      await this.unpack(key, job, live);
    } catch (error) {
      if (!live()) {
        return;
      }
      const problem = problemOf(error);
      if (closesImport(problem)) {
        this.close(problem);
      } else {
        this.patch(key, (row) => ({ ...row, phase: { kind: 'failed', problem } }));
      }
    } finally {
      if (held.controller === controller) {
        held.controller = null;
      }
      this.pump();
    }
  }

  /** Sends the file from the server's offset to its end; the unzip job, when the server starts one. */
  private async transfer(key: string, held: Held, signal: AbortSignal): Promise<number | null> {
    const file = held.file;
    if (file === null) {
      throw new Error(`no bytes for ${key}`);
    }
    let offset = await this.open(key, held, file);
    let job: number | null = null;
    let failures = 0;
    let resync = false;
    for (;;) {
      try {
        if (resync) {
          offset = await this.serverOffset(held);
          resync = false;
          this.setSent(key, offset);
        }
        if (offset >= file.size) {
          return job;
        }
        const chunk = await this.api.append(this.importId, this.uploadOf(held), offset, file.slice(offset, offset + this.chunkSize), signal);
        offset = chunk.offset;
        job = chunk.unzipJob ?? job;
        failures = 0;
        this.setSent(key, offset);
      } catch (error) {
        const recoverable = refusalOf(error)?.code === 'offsetConflict' || error instanceof TypeError;
        if (signal.aborted || !recoverable || failures >= this.retries) {
          throw error;
        }
        failures += 1;
        if (error instanceof TypeError) {
          await this.wait(Math.min(8000, 1000 * 2 ** (failures - 1)));
        }
        resync = true;
      }
    }
  }

  /** The offset to send from: a resumed upload's, or 0 for a new one. */
  private async open(key: string, held: Held, file: File): Promise<number> {
    if (held.upload !== null) {
      try {
        const head = await this.api.offset(this.importId, held.upload);
        if (head.length === file.size) {
          return head.offset;
        }
        // The same path, other bytes: the unfinished upload is not this file's.
        await this.api.remove(this.importId, held.upload);
      } catch (error) {
        if (refusalOf(error)?.code !== 'gone') {
          throw error;
        }
      }
      this.forget(key);
    }
    try {
      held.upload = await this.api.create(this.importId, key, file.size);
      this.set({ used: this.state.used + file.size });
      return 0;
    } catch (error) {
      const refusal = refusalOf(error);
      if (refusal?.code !== 'uploadInProgress') {
        throw error;
      }
      held.upload = refusal.upload;
      const head = await this.api.offset(this.importId, refusal.upload);
      if (head.length !== file.size) {
        await this.api.remove(this.importId, refusal.upload);
        held.upload = await this.api.create(this.importId, key, file.size);
        return 0;
      }
      return head.offset;
    }
  }

  private async unpack(key: string, job: number, live: () => boolean): Promise<void> {
    for (;;) {
      await this.wait(POLL_MS);
      if (!live()) {
        return;
      }
      const status = await this.api.jobStatus(job);
      if (status === 'completed') {
        // Read the unpacked files before the row says done, so the counts arrive with it.
        this.held.delete(key);
        await this.load();
        this.patch(key, (row) => ({ ...row, phase: { kind: 'done', unpacked: true } }));
        this.onFilesChanged();
        return;
      }
      if (status === 'error' || status === 'stopped') {
        const reason = await this.api.jobReason(job);
        this.patch(key, (row) => ({ ...row, phase: { kind: 'failed', problem: { kind: 'zipRefused', reason } } }));
        return;
      }
    }
  }

  private async serverOffset(held: Held): Promise<number> {
    return (await this.api.offset(this.importId, this.uploadOf(held))).offset;
  }

  private uploadOf(held: Held): string {
    if (held.upload === null) {
      throw new Error('a transfer without an upload');
    }
    return held.upload;
  }

  private forget(key: string): void {
    const held = this.held.get(key);
    if (held !== undefined) {
      held.upload = null;
    }
  }

  private close(problem: UploadProblem): void {
    for (const held of this.held.values()) {
      held.controller?.abort();
    }
    this.patchAll((row) => (ACTIVE.has(row.phase.kind) || row.phase.kind === 'paused' ? { ...row, phase: { kind: 'failed', problem } } : row));
    this.set({ closed: problem });
  }

  private setSent(key: string, sent: number): void {
    this.patch(key, (row) => (row.sent === sent ? row : { ...row, sent }));
  }

  private patch(key: string, change: (row: UploadRow) => UploadRow): void {
    this.patchAll((row) => (row.key === key ? change(row) : row));
  }

  private patchAll(change: (row: UploadRow) => UploadRow): void {
    const before = this.state.rows;
    const rows = before.map(change);
    if (rows.some((row, index) => row !== before[index])) {
      this.set({ rows });
    }
  }

  private set(change: Partial<UploadSnapshot>): void {
    const next = { ...this.state, ...change };
    let count = this.earlier.size;
    let bytes = 0;
    for (const stored of this.earlier.values()) {
      bytes += stored.length;
    }
    for (const row of next.rows) {
      if (row.phase.kind === 'done' && !row.phase.unpacked) {
        count += 1;
        bytes += row.size;
      }
    }
    this.state = { ...next, stored: { count, bytes } };
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export interface Totals {
  readonly sent: number;
  readonly size: number;
  readonly done: number;
  readonly count: number;
}

/** The overall progress of the rows this page sends. */
export const totals = (rows: readonly UploadRow[]): Totals => {
  let sent = 0;
  let size = 0;
  let done = 0;
  for (const row of rows) {
    sent += row.sent;
    size += row.size;
    done += row.phase.kind === 'done' ? 1 : 0;
  }
  return { sent, size, done, count: rows.length };
};
