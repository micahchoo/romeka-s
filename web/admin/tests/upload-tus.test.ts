/**
 * The shared tus wire (`app/upload/tus.ts`) and the import's job routes
 * (`api/import-uploads.ts`): the listing parse, the refusals it types, and the
 * job a zip reports through.
 */
import { describe, expect, it } from 'vitest';
import { ImportJobs } from '../src/api/import-uploads';
import { ApiError } from '../src/api/client';
import { ParseError } from '../src/api/parse';
import { TusUploads, UploadRefusedError, encodeFilename, refusalOf } from '../src/app/upload/tus';

const BASE = '/api/romeka/imports/3/uploads';

const answering = (response: () => Response): TusUploads =>
  new TusUploads({ basePath: BASE, baseUrl: '', fetchFn: () => Promise.resolve(response()) });

const json = (status: number, body: unknown): Response => new Response(JSON.stringify(body), { status });

const refusalFrom = async (status: number, body: unknown): Promise<unknown> => {
  try {
    await answering(() => json(status, body)).create('a.tif', 1);
  } catch (error) {
    return error;
  }
  throw new Error('expected a refusal');
};

describe('the listing', () => {
  it('reads uploads, the bytes used and the limits', async () => {
    const body = {
      uploads: [{ id: 7, path: 'Box/a.tif', length: 30, received: 10 }],
      used: 30,
      limit: { file: 100, total: 1000 },
      keepUntil: '2026-10-27T12:00:00+00:00',
    };
    expect(await answering(() => json(200, body)).list()).toEqual({
      uploads: [{ id: '7', path: 'Box/a.tif', length: 30, received: 10 }],
      used: 30,
      limit: { file: 100, total: 1000 },
      keepUntil: '2026-10-27T12:00:00+00:00',
    });
  });

  it('reads a collection that states neither a total nor a limit', async () => {
    const body = { uploads: [{ id: 'u1', path: 'a.jpg', mediaType: 'image/jpeg', itemId: 7, length: 3, received: 3, created: 'x', expires: 'y' }] };
    expect(await answering(() => json(200, body)).list()).toEqual({
      uploads: [{ id: 'u1', path: 'a.jpg', length: 3, received: 3 }],
      used: null,
      limit: null,
      keepUntil: null,
    });
  });

  it('refuses a listing with a member it does not know', async () => {
    const body = { uploads: [], used: 0, limit: { file: 1, total: 1 }, extra: true };
    await expect(answering(() => json(200, body)).list()).rejects.toBeInstanceOf(ParseError);
  });
});

describe('refusals', () => {
  it.each([
    [400, { code: 'uploadPath' }, { code: 'uploadPath' }],
    [413, { code: 'fileTooLarge', limit: 5 }, { code: 'fileTooLarge', limit: 5 }],
    [413, { code: 'importFull', limit: 9, used: 8 }, { code: 'importFull', limit: 9, used: 8 }],
    [409, { code: 'uploadInProgress', upload: 'u4' }, { code: 'uploadInProgress', upload: 'u4' }],
    [409, { code: 'notDraft', status: 'done' }, { code: 'notDraft' }],
    [403, {}, { code: 'forbidden' }],
    [404, {}, { code: 'gone' }],
  ])('reads a %i %j', async (status, members, expected) => {
    const error = await refusalFrom(status, { errors: { error: 'refused' }, ...members });
    expect(error).toBeInstanceOf(UploadRefusedError);
    expect(refusalOf(error)).toEqual(expected);
  });

  it('keeps a refusal whose members it does not know as a plain error with the server text', async () => {
    const error = await refusalFrom(413, { errors: { error: 'Too big.' }, code: 'fileTooLarge', limit: 5, extra: 1 });
    expect(error).toBeInstanceOf(ApiError);
    expect(refusalOf(error)).toBeNull();
    expect(error).toHaveProperty('message', 'Too big.');
  });
});

describe('the wire', () => {
  it('encodes the path as base64 of its UTF-8 bytes', () => {
    expect(encodeFilename('Caja ñ/0001.tif')).toBe(`filename ${Buffer.from('Caja ñ/0001.tif', 'utf8').toString('base64')}`);
  });

  it('reads the upload id from Location and refuses a Location outside this collection', async () => {
    const at = (location: string): TusUploads => answering(() => new Response(null, { status: 201, headers: { Location: location } }));
    expect(await at(`${BASE}/u9`).create('a.tif', 1)).toBe('u9');
    await expect(at('/api/romeka/imports/4/uploads/u9').create('a.tif', 1)).rejects.toBeInstanceOf(ParseError);
  });

  it('reads the unzip job from the last PATCH and the reason from the last line of its log', async () => {
    const patched = answering(() => new Response(null, { status: 204, headers: { 'Upload-Offset': '15', 'Romeka-Unzip-Job': '12' } }));
    expect(await patched.append('u1', 10, new Blob(['12345']), undefined, 'Romeka-Unzip-Job')).toEqual({ offset: 15, extension: '12' });
    const jobs = new ImportJobs({
      baseUrl: '',
      fetchFn: (input) => {
        const path = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
        return Promise.resolve(
          path.startsWith('/api/jobs/')
            ? json(200, { 'o:id': 12, 'o:status': 'in_progress' })
            : new Response('Started.\nToo many files in the zip file.\n\n', { status: 200 }),
        );
      },
    });
    expect(await jobs.jobReason(12)).toBe('Too many files in the zip file.');
    expect(await jobs.jobStatus(12)).toBe('in_progress');
  });
});
