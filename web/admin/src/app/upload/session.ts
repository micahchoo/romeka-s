/**
 * The files a person adds to one destination from this computer, and the
 * queue that sends them: a few at a time, in chunks, resumable from the offset
 * the server holds.
 *
 * Each file is a row whose `phase` is the one thing it is doing. The bytes,
 * the server's upload id and the abort handle stay out of the snapshot, so a
 * view renders rows without reaching into the transfer. No React here: the
 * view subscribes (`useSyncExternalStore`), and a test drives it directly.
 *
 * What a destination decides (its routes, its limits, its refusal words,
 * whether it unpacks a zip) is behind `UploadTarget`.
 */
import type { UploadProblem } from './problem';
import type { Picked } from './pick';
import type { StoredUpload, UploadLimit } from './tus';
import { refusalOf } from './tus';
import type { SentChunk, Unpack, UploadTarget } from './target';

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
  readonly limit: UploadLimit | null;
  readonly used: number;
  /** Dot files and dot folders left out of what was added. */
  readonly skipped: number;
  /** Files added again whose path and size a stored file already has. */
  readonly alreadyHere: number;
  /** Set when no file can be added to this destination any more. */
  readonly closed: UploadProblem | null;
  readonly loadProblem: UploadProblem | null;
}

export interface UploadSessionOptions {
  readonly target: UploadTarget;
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
  private readonly target: UploadTarget;
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
    this.target = options.target;
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

  /** The server's id for a row's bytes, once it started receiving them. */
  readonly uploadOf = (key: string): string | null => this.held.get(key)?.upload ?? null;

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
      await this.target.remove(upload);
    } catch (error) {
      const problem = this.target.problemOf(error);
      if (problem.kind !== 'gone') {
        const rows = [...this.state.rows];
        rows.splice(index, 0, { ...row, phase: { kind: 'failed', problem } });
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
      const [listing, stated] = await Promise.all([
        this.target.list(),
        this.target.limits === undefined ? Promise.resolve(null) : this.target.limits(),
      ]);
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
      const limit = listing.limit ?? stated;
      this.set({ loaded: true, rows, limit, used: listing.used, loadProblem: null });
    } catch (error) {
      const problem = this.target.problemOf(error);
      this.set({ loaded: true, loadProblem: problem, closed: this.target.closes(problem) ? problem : this.state.closed });
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
      const problem = this.target.problemOf(error);
      if (this.target.closes(problem)) {
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

  /** Sends the file from the server's offset to its end; the unpack job, when the server starts one. */
  private async transfer(key: string, held: Held, signal: AbortSignal): Promise<Unpack | null> {
    const file = held.file;
    if (file === null) {
      throw new Error(`no bytes for ${key}`);
    }
    let offset = await this.open(key, held, file);
    let job: Unpack | null = null;
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
        const chunk: SentChunk = await this.target.append(this.uploadId(held), offset, file.slice(offset, offset + this.chunkSize), signal);
        offset = chunk.offset;
        job = chunk.unpack ?? job;
        failures = 0;
        this.setSent(key, offset);
      } catch (error) {
        const refusal = refusalOf(error);
        const recoverable = refusal?.code === 'offsetConflict' || error instanceof TypeError;
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
        const head = await this.target.offset(held.upload);
        if (head.length === file.size) {
          return head.offset;
        }
        // The same path, other bytes: the unfinished upload is not this file's.
        await this.target.remove(held.upload);
      } catch (error) {
        if (refusalOf(error)?.code !== 'gone') {
          throw error;
        }
      }
      this.forget(key);
    }
    try {
      held.upload = await this.target.create(key, file.size, file.type === '' ? null : file.type);
      this.set({ used: this.state.used + file.size });
      return 0;
    } catch (error) {
      const refusal = refusalOf(error);
      if (refusal?.code !== 'uploadInProgress') {
        throw error;
      }
      held.upload = refusal.upload;
      const head = await this.target.offset(refusal.upload);
      if (head.length !== file.size) {
        await this.target.remove(refusal.upload);
        held.upload = await this.target.create(key, file.size, file.type === '' ? null : file.type);
        return 0;
      }
      return head.offset;
    }
  }

  private async unpack(key: string, job: Unpack, live: () => boolean): Promise<void> {
    for (;;) {
      await this.wait(POLL_MS);
      if (!live()) {
        return;
      }
      const state = await job.check();
      if (state.kind === 'done') {
        // Read the unpacked files before the row says done, so the counts arrive with it.
        this.held.delete(key);
        await this.load();
        this.patch(key, (row) => ({ ...row, phase: { kind: 'done', unpacked: true } }));
        this.onFilesChanged();
        return;
      }
      if (state.kind === 'failed') {
        this.patch(key, (row) => ({ ...row, phase: { kind: 'failed', problem: state.problem } }));
        return;
      }
    }
  }

  private async serverOffset(held: Held): Promise<number> {
    return (await this.target.offset(this.uploadId(held))).offset;
  }

  private uploadId(held: Held): string {
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
