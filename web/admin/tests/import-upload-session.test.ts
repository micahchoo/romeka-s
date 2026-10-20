/**
 * The upload queue (`pages/import/upload/session.ts`) through the real client
 * (`api/import-uploads.ts`) against an in-memory tus 1.0 server that answers
 * the way `docs/import-v2.md` ("Arrival", "Wire") says the routes do.
 */
import { describe, expect, it } from 'vitest';
import { UploadApi } from '../src/api/import-uploads';
import type { JobStatus } from '../src/api/import-uploads';
import { brandId } from '../src/api/parse';
import { UploadSession, totals } from '../src/pages/import/upload/session';
import type { UploadSnapshot } from '../src/pages/import/upload/session';
import type { PickedFile } from '../src/pages/import/upload/pick';

const ID = brandId<'ImportId'>(4);
const BASE = '/api/romeka/imports/4/uploads';
const CHUNK = 10;

interface Stored {
  path: string;
  length: number;
  offset: number;
}

type Refusal = { readonly status: number; readonly body: Record<string, unknown> };

const json = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

const refused = ({ status, body }: Refusal): Response => json(status, { errors: { error: 'refused' }, ...body });

const bodySize = (init: RequestInit | undefined): number => (init?.body instanceof Blob ? init.body.size : 0);

const headerOf = (init: RequestInit | undefined, name: string): string | null => new Headers(init?.headers).get(name);

/** A tus server in memory. Each hook answers one request, then clears, so a test states one fault. */
class FakeServer {
  readonly uploads = new Map<string, Stored>();
  readonly requests: { method: string; path: string; init: RequestInit | undefined }[] = [];
  limit = { file: 1000, total: 10_000 };
  next = 1;
  /** The answer to the next POST, instead of creating. */
  refuseCreate: Refusal | null = null;
  /** The next PATCH stores its chunk, then the connection drops before the answer. */
  dropAfterStore = false;
  /** Before the PATCH numbered `at` (from 1), another client moves the stored offset to `to`. */
  moveOffset: { readonly at: number; readonly to: number } | null = null;
  /** While set, a PATCH waits until released or aborted. */
  hold: { release: () => void; promise: Promise<void> } | null = null;
  private patchCount = 0;
  private entered: (() => void) | null = null;
  inFlight = 0;
  mostInFlight = 0;
  jobs = new Map<number, { statuses: JobStatus[]; log: string; onComplete?: () => void }>();
  readonly fetch: typeof fetch = async (input, init) => {
    const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url, 'http://admin.test');
    const method = init?.method ?? 'GET';
    this.requests.push({ method, path: url.pathname, init });
    return this.answer(method, url.pathname, init);
  };

  seed(id: string, stored: Stored): void {
    this.uploads.set(id, stored);
  }

  patches(): string[] {
    return this.requests.filter((r) => r.method === 'PATCH').map((r) => `${r.path.replace(`${BASE}/`, '')}@${headerOf(r.init, 'Upload-Offset') ?? ''}`);
  }

  methods(): string[] {
    return this.requests.map((r) => r.method);
  }

  holdPatches(): void {
    let release = (): void => undefined;
    const promise = new Promise<void>((resolve) => {
      release = resolve;
    });
    this.hold = { release, promise };
  }

  private async answer(method: string, path: string, init: RequestInit | undefined): Promise<Response> {
    if (path.startsWith('/api/jobs/')) {
      const job = this.jobs.get(Number(path.slice('/api/jobs/'.length)));
      const status = job?.statuses.shift() ?? 'completed';
      if (status === 'completed') {
        job?.onComplete?.();
      }
      return json(200, { 'o:id': 1, 'o:status': status });
    }
    if (path.startsWith('/api/romeka/jobs/')) {
      const job = this.jobs.get(Number(path.split('/')[4]));
      return new Response(job?.log ?? '', { status: 200 });
    }
    if (path === BASE && method === 'GET') {
      const uploads = [...this.uploads].map(([id, u]) => ({ id, path: u.path, length: u.length, received: u.offset }));
      const used = uploads.reduce((sum, u) => sum + u.length, 0);
      return json(200, { uploads, used, limit: this.limit });
    }
    if (path === BASE && method === 'POST') {
      const refusal = this.refuseCreate;
      this.refuseCreate = null;
      if (refusal !== null) {
        return refused(refusal);
      }
      const id = `u${String(this.next++)}`;
      const metadata = headerOf(init, 'Upload-Metadata') ?? '';
      const bytes = Uint8Array.from(atob(metadata.replace('filename ', '')), (c) => c.charCodeAt(0));
      this.uploads.set(id, { path: new TextDecoder().decode(bytes), length: Number(headerOf(init, 'Upload-Length')), offset: 0 });
      return new Response(null, { status: 201, headers: { Location: `${BASE}/${id}`, 'Tus-Resumable': '1.0.0' } });
    }
    const id = path.slice(BASE.length + 1);
    const stored = this.uploads.get(id);
    if (stored === undefined) {
      return json(404, { errors: { error: 'Not found' } });
    }
    if (method === 'HEAD') {
      return new Response(null, { status: 200, headers: { 'Upload-Offset': String(stored.offset), 'Upload-Length': String(stored.length) } });
    }
    if (method === 'DELETE') {
      this.uploads.delete(id);
      return new Response(null, { status: 204 });
    }
    this.patchCount += 1;
    if (this.moveOffset?.at === this.patchCount) {
      stored.offset = this.moveOffset.to;
    }
    this.inFlight += 1;
    this.mostInFlight = Math.max(this.mostInFlight, this.inFlight);
    try {
      await this.wait(init?.signal ?? null);
    } finally {
      this.inFlight -= 1;
    }
    if (Number(headerOf(init, 'Upload-Offset')) !== stored.offset) {
      return refused({ status: 409, body: { code: 'offsetConflict' } });
    }
    stored.offset += bodySize(init);
    if (this.dropAfterStore) {
      this.dropAfterStore = false;
      throw new TypeError('Failed to fetch');
    }
    const headers: Record<string, string> = { 'Upload-Offset': String(stored.offset) };
    if (stored.offset === stored.length && stored.path.endsWith('.zip')) {
      headers['Romeka-Unzip-Job'] = '9';
    }
    return new Response(null, { status: 204, headers });
  }

  /** Resolves once a PATCH waits on the hold. */
  held(): Promise<void> {
    return new Promise((resolve) => {
      this.entered = resolve;
    });
  }

  private async wait(signal: AbortSignal | null): Promise<void> {
    await Promise.resolve();
    const hold = this.hold;
    if (hold === null) {
      return;
    }
    this.entered?.();
    this.entered = null;
    await new Promise<void>((resolve, reject) => {
      signal?.addEventListener('abort', () => {
        reject(new DOMException('The operation was aborted.', 'AbortError'));
      });
      void hold.promise.then(resolve);
    });
  }
}

const file = (path: string, size: number): PickedFile => ({ path, file: new File([new Uint8Array(size)], path.split('/').at(-1) ?? path) });

const setup = (server = new FakeServer(), concurrency = 3) => {
  const changes: string[] = [];
  const session = new UploadSession({
    api: new UploadApi({ baseUrl: '', fetchFn: server.fetch }),
    importId: ID,
    chunkSize: CHUNK,
    concurrency,
    wait: () => Promise.resolve(),
    onFilesChanged: () => changes.push('changed'),
  });
  const seen: UploadSnapshot[] = [];
  session.subscribe(() => seen.push(session.snapshot()));
  return { server, session, seen, changes };
};

const phases = (snapshot: UploadSnapshot): Record<string, string> =>
  Object.fromEntries(snapshot.rows.map((row) => [row.path, row.phase.kind === 'failed' ? `failed:${row.phase.problem.kind}` : row.phase.kind]));

/** Resolves once `test` holds for the session's snapshot. */
const until = (session: UploadSession, test: (snapshot: UploadSnapshot) => boolean): Promise<void> =>
  new Promise((resolve) => {
    const check = (): void => {
      if (test(session.snapshot())) {
        stop();
        resolve();
      }
    };
    const stop = session.subscribe(check);
    check();
  });

describe('sending', () => {
  it('sends each file in chunks with the tus headers and reports progress per file and overall', async () => {
    const { server, session, seen, changes } = setup();
    await session.add({ files: [file('Box 1/0001.tif', 25), file('Caja ñ/a.jpg', 10)], skipped: 2 });
    await session.whenIdle();

    expect([...server.uploads.values()]).toEqual([
      { path: 'Box 1/0001.tif', length: 25, offset: 25 },
      { path: 'Caja ñ/a.jpg', length: 10, offset: 10 },
    ]);
    const post = server.requests.find((r) => r.method === 'POST');
    expect(headerOf(post?.init, 'Tus-Resumable')).toBe('1.0.0');
    expect(headerOf(post?.init, 'Upload-Length')).toBe('25');
    expect(headerOf(post?.init, 'Upload-Metadata')).toBe(`filename ${btoa('Box 1/0001.tif')}`);
    const patch = server.requests.find((r) => r.method === 'PATCH');
    expect(headerOf(patch?.init, 'Content-Type')).toBe('application/offset+octet-stream');
    expect(server.patches()).toEqual(['u1@0', 'u2@0', 'u1@10', 'u1@20']);

    const tif = seen.map((s) => s.rows.find((row) => row.path === 'Box 1/0001.tif')?.sent);
    expect([...new Set(tif.filter((sent) => sent !== undefined))]).toEqual([0, 10, 20, 25]);
    const overall = seen.map((s) => totals(s.rows));
    expect(overall.at(-1)).toEqual({ sent: 35, size: 35, done: 2, count: 2 });
    expect(overall.some((t) => t.sent > 0 && t.sent < 35)).toBe(true);
    expect(phases(session.snapshot())).toEqual({ 'Box 1/0001.tif': 'done', 'Caja ñ/a.jpg': 'done' });
    expect(session.snapshot().stored).toEqual({ count: 2, bytes: 35 });
    expect(session.snapshot().skipped).toBe(2);
    expect(changes).toEqual(['changed', 'changed']);
  });

  it('sends at most the concurrency at once', async () => {
    const { server, session } = setup(new FakeServer(), 2);
    await session.add({ files: ['a', 'b', 'c', 'd', 'e'].map((name) => file(`${name}.tif`, 30)), skipped: 0 });
    await session.whenIdle();
    expect(server.mostInFlight).toBe(2);
    expect(totals(session.snapshot().rows)).toEqual({ sent: 150, size: 150, done: 5, count: 5 });
  });
});

describe('resuming', () => {
  it('asks the server for its offset after a dropped PATCH and continues from there, sending no byte twice', async () => {
    const { server, session } = setup();
    server.dropAfterStore = true;
    await session.add({ files: [file('a.tif', 25)], skipped: 0 });
    await session.whenIdle();
    expect(server.methods()).toEqual(['GET', 'POST', 'PATCH', 'HEAD', 'PATCH', 'PATCH']);
    expect(server.patches()).toEqual(['u1@0', 'u1@10', 'u1@20']);
    expect(phases(session.snapshot())).toEqual({ 'a.tif': 'done' });
  });

  it('answers a 409 offsetConflict with a HEAD and continues from the answered offset', async () => {
    const { server, session } = setup();
    server.moveOffset = { at: 2, to: 20 };
    await session.add({ files: [file('a.tif', 30)], skipped: 0 });
    await session.whenIdle();
    expect(server.patches()).toEqual(['u1@0', 'u1@10', 'u1@20']);
    expect(server.methods().slice(-3)).toEqual(['PATCH', 'HEAD', 'PATCH']);
    expect(server.uploads.get('u1')?.offset).toBe(30);
  });

  it('pauses one file mid-chunk and resumes it from the offset the server holds', async () => {
    const { server, session } = setup();
    server.holdPatches();
    const held = server.held();
    await session.add({ files: [file('a.tif', 25)], skipped: 0 });
    await held;
    session.pause('a.tif');
    expect(phases(session.snapshot())).toEqual({ 'a.tif': 'paused' });
    await session.whenIdle();
    expect(server.uploads.get('u1')?.offset).toBe(0);

    server.hold = null;
    session.resume('a.tif');
    await session.whenIdle();
    expect(server.methods()).toEqual(['GET', 'POST', 'PATCH', 'HEAD', 'PATCH', 'PATCH', 'PATCH']);
    expect(phases(session.snapshot())).toEqual({ 'a.tif': 'done' });
  });

  it('pauses and resumes every file at once', async () => {
    const { server, session } = setup(new FakeServer(), 1);
    server.holdPatches();
    const held = server.held();
    await session.add({ files: [file('a.tif', 10), file('b.tif', 10)], skipped: 0 });
    await held;
    session.pauseAll();
    expect(phases(session.snapshot())).toEqual({ 'a.tif': 'paused', 'b.tif': 'paused' });
    server.hold = null;
    session.resumeAll();
    await session.whenIdle();
    expect(phases(session.snapshot())).toEqual({ 'a.tif': 'done', 'b.tif': 'done' });
  });

  it('resumes an unfinished upload from an earlier visit when the same path and size is added again', async () => {
    const server = new FakeServer();
    server.seed('u7', { path: 'Box/a.tif', length: 30, offset: 10 });
    server.seed('u8', { path: 'Box/b.tif', length: 20, offset: 20 });
    const { session } = setup(server);
    await session.load();
    expect(phases(session.snapshot())).toEqual({ 'Box/a.tif': 'needsFile' });
    expect(session.snapshot().stored).toEqual({ count: 1, bytes: 20 });
    expect(session.snapshot().used).toBe(50);

    await session.add({ files: [file('Box/a.tif', 30), file('Box/b.tif', 20)], skipped: 0 });
    await session.whenIdle();
    expect(server.patches()).toEqual(['u7@10', 'u7@20']);
    expect(session.snapshot().alreadyHere).toBe(1);
    expect(phases(session.snapshot())).toEqual({ 'Box/a.tif': 'done' });
  });

  it('starts over when the unfinished upload at that path has another size', async () => {
    const server = new FakeServer();
    server.seed('u7', { path: 'a.tif', length: 30, offset: 10 });
    const { session } = setup(server);
    await session.add({ files: [file('a.tif', 12)], skipped: 0 });
    await session.whenIdle();
    expect(server.methods()).toEqual(['GET', 'HEAD', 'DELETE', 'POST', 'PATCH', 'PATCH']);
    expect([...server.uploads.values()]).toEqual([{ path: 'a.tif', length: 12, offset: 12 }]);
  });

  it('resumes the upload a 409 uploadInProgress names', async () => {
    const server = new FakeServer();
    server.uploads.set('u5', { path: 'a.tif', length: 20, offset: 10 });
    server.refuseCreate = { status: 409, body: { code: 'uploadInProgress', upload: 'u5' } };
    const { session } = setup(server);
    // A listing from before the other tab started: the row is new here.
    server.uploads.delete('u5');
    await session.load();
    server.uploads.set('u5', { path: 'a.tif', length: 20, offset: 10 });
    await session.add({ files: [file('a.tif', 20)], skipped: 0 });
    await session.whenIdle();
    expect(server.methods()).toEqual(['GET', 'POST', 'HEAD', 'PATCH']);
    expect(server.patches()).toEqual(['u5@10']);
  });
});

describe('refusals', () => {
  it('reads fileTooLarge, importFull and uploadPath as problems of that file only', async () => {
    const { server, session } = setup();
    await session.load();
    server.refuseCreate = { status: 413, body: { code: 'fileTooLarge', limit: 5 } };
    await session.add({ files: [file('big.tif', 8)], skipped: 0 });
    await session.whenIdle();
    server.refuseCreate = { status: 413, body: { code: 'importFull', limit: 100, used: 95 } };
    await session.add({ files: [file('full.tif', 8)], skipped: 0 });
    await session.whenIdle();
    server.refuseCreate = { status: 400, body: { code: 'uploadPath' } };
    await session.add({ files: [file('a/b.tif', 8)], skipped: 0 });
    await session.whenIdle();

    const rows = session.snapshot().rows.map((row) => row.phase);
    expect(rows).toEqual([
      { kind: 'failed', problem: { kind: 'fileTooLarge', limit: 5 } },
      { kind: 'failed', problem: { kind: 'importFull', limit: 100, used: 95 } },
      { kind: 'failed', problem: { kind: 'badName' } },
    ]);
    expect(session.snapshot().closed).toBeNull();
  });

  it('refuses a file over the per-file limit before sending anything', async () => {
    const server = new FakeServer();
    server.limit = { file: 5, total: 100 };
    const { session } = setup(server);
    await session.add({ files: [file('big.tif', 6)], skipped: 0 });
    await session.whenIdle();
    expect(server.methods()).toEqual(['GET']);
    expect(session.snapshot().rows[0]?.phase).toEqual({ kind: 'failed', problem: { kind: 'fileTooLarge', limit: 5 } });
  });

  it('closes the import on notDraft: every file still to send fails, and new files are refused', async () => {
    const { server, session } = setup(new FakeServer(), 1);
    await session.load();
    server.refuseCreate = { status: 409, body: { code: 'notDraft', status: 'done' } };
    await session.add({ files: [file('a.tif', 5), file('b.tif', 5)], skipped: 0 });
    await session.whenIdle();
    expect(session.snapshot().closed).toEqual({ kind: 'importStarted' });
    expect(phases(session.snapshot())).toEqual({ 'a.tif': 'failed:importStarted', 'b.tif': 'failed:importStarted' });
    await session.add({ files: [file('c.tif', 5)], skipped: 0 });
    expect(phases(session.snapshot())['c.tif']).toBe('failed:importStarted');
    expect(server.methods().filter((m) => m === 'POST')).toHaveLength(1);
  });

  it('reads a refused zip from the last line of its job log', async () => {
    const { server, session, changes } = setup();
    server.jobs.set(9, { statuses: ['in_progress', 'error'], log: 'Started.\nA file inside would land outside the folder: ../etc/passwd\n' });
    await session.add({ files: [file('box.zip', 15)], skipped: 0 });
    await until(session, (s) => s.rows[0]?.phase.kind === 'unpacking');
    await session.whenIdle();
    expect(session.snapshot().rows[0]?.phase).toEqual({
      kind: 'failed',
      problem: { kind: 'zipRefused', reason: 'A file inside would land outside the folder: ../etc/passwd' },
    });
    expect(changes).toEqual([]);
  });

  it('marks an unpacked zip done and reads the files it left', async () => {
    const { server, session, changes } = setup();
    const unpack = (): void => {
      server.uploads.delete('u1');
      server.seed('u2', { path: 'box/a.tif', length: 40, offset: 40 });
    };
    server.jobs.set(9, { statuses: ['in_progress', 'completed'], log: '', onComplete: unpack });
    await session.add({ files: [file('box.zip', 15)], skipped: 0 });
    await session.whenIdle();
    expect(session.snapshot().rows[0]?.phase).toEqual({ kind: 'done', unpacked: true });
    expect(session.snapshot().stored).toEqual({ count: 1, bytes: 40 });
    expect(changes).toEqual(['changed']);
  });

  it('starts a file over from the first byte on a retry after the server lost it', async () => {
    const { server, session } = setup();
    await session.add({ files: [file('a.tif', 25)], skipped: 0 });
    await until(session, (s) => s.rows[0]?.sent === 10);
    server.uploads.delete('u1');
    await session.whenIdle();
    expect(session.snapshot().rows[0]?.phase).toEqual({ kind: 'failed', problem: { kind: 'gone' } });
    session.retry('a.tif');
    await session.whenIdle();
    expect([...server.uploads]).toEqual([['u2', { path: 'a.tif', length: 25, offset: 25 }]]);
  });

  it('fails a file after the connection drops more times than it retries', async () => {
    const server = new FakeServer();
    const failing: typeof fetch = (input, init) => (init?.method === 'PATCH' ? Promise.reject(new TypeError('Failed to fetch')) : server.fetch(input, init));
    const flaky = new UploadSession({ api: new UploadApi({ baseUrl: '', fetchFn: failing }), importId: ID, chunkSize: CHUNK, retries: 2, wait: () => Promise.resolve() });
    await flaky.add({ files: [file('a.tif', 25)], skipped: 0 });
    await flaky.whenIdle();
    expect(flaky.snapshot().rows[0]?.phase).toEqual({ kind: 'failed', problem: { kind: 'connection' } });
    expect(server.methods()).toEqual(['GET', 'POST', 'HEAD', 'HEAD']);
  });
});

describe('removing', () => {
  it('removes a sent file from the server and says the files changed', async () => {
    const { server, session, changes } = setup();
    await session.add({ files: [file('a.tif', 5)], skipped: 0 });
    await session.whenIdle();
    await session.remove('a.tif');
    expect(server.uploads.size).toBe(0);
    expect(session.snapshot().rows).toEqual([]);
    expect(session.snapshot().stored).toEqual({ count: 0, bytes: 0 });
    expect(changes).toEqual(['changed', 'changed']);
  });

  it('removes an unfinished upload from an earlier visit', async () => {
    const server = new FakeServer();
    server.seed('u7', { path: 'a.tif', length: 30, offset: 10 });
    const { session, changes } = setup(server);
    await session.load();
    await session.remove('a.tif');
    expect(server.methods()).toEqual(['GET', 'DELETE']);
    expect(server.uploads.size).toBe(0);
    expect(changes).toEqual([]);
  });
});
