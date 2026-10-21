/**
 * A person's staged uploads (`/api/romeka/uploads`) and the media create that
 * names one: what the add panel reads and sends. The routes are S4's
 * (`briefs/media-ui-plan.md`); the shapes are `crates/omeka-server/README.md`,
 * "Staged uploads".
 */
import { api } from './client';
import type { ItemId, Media } from './types';
import { asCount, list, member, strict } from './import-read';
import { ParseError, asBoolean, asString, parseMedia } from './parse';
import { problemOf } from '../app/upload/problem';
import type { UploadProblem } from '../app/upload/problem';
import type { SentChunk, UploadListing, UploadTarget } from '../app/upload/target';
import { TusUploads, parseTusListing } from '../app/upload/tus';
import type { UploadLimit } from '../app/upload/tus';

/** The readers the probe can answer, as `omeka_api::media_add::Probe` names them. */
export const PROBE_INGESTERS = ['youtube', 'oembed', 'iiif', 'iiif_presentation', 'url'] as const;
export type ProbeIngester = (typeof PROBE_INGESTERS)[number];
/** Every ingester the panel offers, including the one no probe names. */
export type Ingester = ProbeIngester | 'html';

/** The file a destination takes, and what it validates. */
export interface UploadLimits {
  readonly maxFileBytes: number;
  readonly kinds: readonly string[];
  readonly extensions: readonly string[];
  readonly validationOff: boolean;
}

/** The limits read: a sentence, or the server's refusal. */
export type LimitsState =
  | { readonly kind: 'ready'; readonly limits: UploadLimits }
  | { readonly kind: 'failed' };

const isProbeIngester = (value: string): value is ProbeIngester =>
  PROBE_INGESTERS.some((reader) => reader === value);

const readIngester = (value: unknown, path: readonly (string | number)[]): ProbeIngester => {
  const name = asString(value, [...path]);
  if (!isProbeIngester(name)) {
    throw new ParseError(path, `one of ${PROBE_INGESTERS.join(', ')}`);
  }
  return name;
};

const readStringList = (value: unknown, path: readonly (string | number)[]): readonly string[] =>
  list(asString)(value, [...path]);

export const parseUploadLimits = (raw: unknown): UploadLimits => {
  const object = strict(raw, [], ['maxFileBytes', 'kinds', 'extensions', 'validationOff']);
  return {
    maxFileBytes: member(object, 'maxFileBytes', [], asCount),
    kinds: member(object, 'kinds', [], readStringList),
    extensions: member(object, 'extensions', [], readStringList),
    validationOff: member(object, 'validationOff', [], asBoolean),
  };
};

/** What a link turned out to be. */
export interface Probe {
  readonly readers: readonly ProbeIngester[];
  readonly suggested: ProbeIngester;
  readonly mediaType: string | null;
  readonly size: number | null;
  readonly title: string | null;
  readonly image: { readonly w: number; readonly h: number } | null;
  readonly canvases: number | null;
}

const maybe = <T>(raw: unknown, path: readonly (string | number)[], parse: (value: unknown, path: readonly (string | number)[]) => T): T | null =>
  raw === null ? null : parse(raw, path);

export const parseProbe = (raw: unknown): Probe => {
  const object = strict(raw, [], ['readers', 'suggested', 'mediaType', 'size', 'title', 'image', 'canvases']);
  const readers = member(object, 'readers', [], (value, path) => list(readIngester)(value, [...path]));
  const first = readers[0];
  if (first === undefined) {
    throw new ParseError(['readers'], 'at least one reader');
  }
  const suggested = member(object, 'suggested', [], readIngester);
  if (suggested !== first) {
    throw new ParseError(['suggested'], `the first reader, ${first}`);
  }
  const image = maybe(object['image'], ['image'], (value, path) => {
    const size = strict(value, [...path], ['w', 'h']);
    return { w: member(size, 'w', [...path], asCount), h: member(size, 'h', [...path], asCount) };
  });
  return {
    readers,
    suggested,
    mediaType: maybe(object['mediaType'], ['mediaType'], asString),
    size: maybe(object['size'], ['size'], asCount),
    title: maybe(object['title'], ['title'], asString),
    image,
    canvases: maybe(object['canvases'], ['canvases'], asCount),
  };
};

/** `GET /api/romeka/upload-limits`: the size and the validation lists. */
export const uploadLimits = async (): Promise<UploadLimits> =>
  parseUploadLimits(await api.requestJson('/api/romeka/upload-limits'));

/** `POST /api/romeka/media/probe {"url"}`: what the link is, with no row written. */
export const probeLink = async (url: string): Promise<Probe> =>
  parseProbe(await api.writeJson('POST', '/api/romeka/media/probe', { url }));

/** Where a new media comes from: bytes a person sent, a link, or pasted HTML. */
export type MediaSource =
  | { readonly kind: 'upload'; readonly upload: string }
  | { readonly kind: 'link'; readonly ingester: Ingester; readonly url: string }
  | { readonly kind: 'html'; readonly html: string };

export interface NewMedia {
  /** Whether visitors see it; the panel starts at the item's own answer. */
  readonly isPublic: boolean;
  readonly altText: string | null;
}

const sourceBody = (source: MediaSource): Record<string, unknown> => {
  switch (source.kind) {
    case 'upload':
      return { 'o:ingester': 'upload', 'o-module-romeka:upload': source.upload };
    case 'link':
      // Each ingester reads its own member: `Ingester\Url` takes `ingest_url`,
      // the others take the media's source.
      return source.ingester === 'url'
        ? { 'o:ingester': 'url', ingest_url: source.url }
        : { 'o:ingester': source.ingester, 'o:source': source.url };
    case 'html':
      return { 'o:ingester': 'html', html: source.html };
  }
};

/**
 * `POST /api/media`. A staged upload's bytes go through the import's streaming
 * path, so a large file never sits in memory; the server removes the upload it
 * claimed once the row is written.
 */
export const createMedia = async (itemId: ItemId, source: MediaSource, media: NewMedia): Promise<Media> =>
  parseMedia(
    await api.writeJson('POST', '/api/media', {
      'o:item': { 'o:id': itemId },
      'o:is_public': media.isPublic,
      ...(media.altText === null ? {} : { 'o:alt_text': media.altText }),
      ...sourceBody(source),
    }),
  );

/** One item's staged uploads, as the add panel's queue. */
export class StagedUploads implements UploadTarget {
  readonly itemId: ItemId;
  private readonly tus: TusUploads;
  private limitsRead: Promise<LimitsState> | null = null;

  constructor(itemId: ItemId, tus: TusUploads = new TusUploads({ basePath: '/api/romeka/uploads', readListing: parseTusListing })) {
    this.itemId = itemId;
    this.tus = tus;
  }

  /** The limits, read once: a sentence for the panel and a size for the queue. */
  readLimits(): Promise<LimitsState> {
    this.limitsRead ??= uploadLimits().then(
      (limits): LimitsState => ({ kind: 'ready', limits }),
      (): LimitsState => ({ kind: 'failed' }),
    );
    return this.limitsRead;
  }

  async limits(): Promise<UploadLimit | null> {
    const state = await this.readLimits();
    return state.kind === 'ready' ? { file: state.limits.maxFileBytes, total: null } : null;
  }

  async list(): Promise<UploadListing> {
    const listing = await this.tus.list({ item: String(this.itemId) });
    const used = listing.uploads.reduce((sum, upload) => sum + upload.length, 0);
    return { uploads: listing.uploads, used, limit: null };
  }

  create(path: string, length: number, mediaType: string | null): Promise<string> {
    const metadata: (readonly [string, string])[] = [['item', String(this.itemId)]];
    if (mediaType !== null) {
      metadata.push(['filetype', mediaType]);
    }
    return this.tus.create(path, length, metadata);
  }

  offset(upload: string): Promise<{ readonly offset: number; readonly length: number }> {
    return this.tus.offset(upload);
  }

  async append(upload: string, offset: number, bytes: Blob, signal?: AbortSignal): Promise<SentChunk> {
    const chunk = await this.tus.append(upload, offset, bytes, signal === undefined ? {} : { signal });
    // A staged upload is never unpacked: it becomes one media's original file.
    return { offset: chunk.offset, unpack: null };
  }

  remove(upload: string): Promise<void> {
    return this.tus.remove(upload);
  }

  problemOf(error: unknown): UploadProblem {
    return problemOf(error);
  }

  /** A 403 means this account may not add media; no file of this item can go. */
  closes(problem: UploadProblem): boolean {
    return problem.kind === 'notAllowed';
  }
}
