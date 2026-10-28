/**
 * Media documents (`/api/media`): the parser, the player URL and the calls.
 *
 * Only a lazy route shows media, so this module stays out of the initial
 * payload `client.ts` is part of, the way `term-lists.ts` and the import
 * families do. The shared parse helpers it reads stay in `parse.ts`, which is
 * eager already.
 */
import type { ApiClient, ResourceQuery } from './client';
import {
  asNumber,
  asRecord,
  asRef,
  asString,
  brandId,
  field,
  nullableField,
  optionalString,
  parseEntityCore,
  parseThumbnailUrls,
  positiveInt,
} from './parse';
import type {
  ApiPage,
  ItemId,
  Media,
  MediaAccess,
  MediaId,
  MediaPreview,
  MediaWrite,
  Parser,
  Path,
  ServerMediaKind,
} from './types';
import { SERVER_MEDIA_KINDS } from './types';
import { encodeMedia } from './write';

/** The preview states the port's `o-module-romeka:preview` may name. */
const PREVIEW_STATES = ['pending', 'ready', 'failed', 'none'] as const;

const isPreviewState = (value: string): value is MediaPreview['state'] =>
  PREVIEW_STATES.some((state) => state === value);

/**
 * The port's derivative state from `o-module-romeka:preview`. A missing member
 * reads as `null` (an older server), and so does a state this build does not
 * know, so a newer server's state never fails the whole document.
 */
const parsePreview = (object: Record<string, unknown>, path: Path): MediaPreview | null => {
  const value = object['o-module-romeka:preview'];
  if (value === undefined || value === null) {
    return null;
  }
  const fieldPath = [...path, 'o-module-romeka:preview'];
  const record = asRecord(value, fieldPath);
  const state = asString(field(record, 'state', fieldPath), [...fieldPath, 'state']);
  if (!isPreviewState(state)) {
    return null;
  }
  const number = record['progress'];
  return {
    state,
    reason: optionalString(record, 'reason', fieldPath),
    progress:
      number === undefined || number === null ? null : asNumber(number, [...fieldPath, 'progress']),
  };
};

/**
 * The port's playable-copy state from `o-module-romeka:access`. A missing
 * member reads as `null` (an older server), and so does a state this build does
 * not know, so a newer server never fails the whole document.
 */
const parseAccess = (object: Record<string, unknown>, path: Path): MediaAccess | null => {
  const value = object['o-module-romeka:access'];
  if (value === undefined || value === null) {
    return null;
  }
  const fieldPath = [...path, 'o-module-romeka:access'];
  const record = asRecord(value, fieldPath);
  const state = asString(field(record, 'state', fieldPath), [...fieldPath, 'state']);
  if (!isPreviewState(state)) {
    return null;
  }
  const number = record['progress'];
  return {
    state,
    reason: optionalString(record, 'reason', fieldPath),
    progress:
      number === undefined || number === null ? null : asNumber(number, [...fieldPath, 'progress']),
    url: optionalString(record, 'url', fieldPath),
    mediaType: optionalString(record, 'media_type', fieldPath),
  };
};

/**
 * The port's kind name from `o:kind`. A name this build does not know reads as
 * absent, so a newer server's kind falls back to the admin's table instead of
 * failing the whole document.
 */
const parseServerKind = (object: Record<string, unknown>, path: Path): ServerMediaKind | null => {
  const value = optionalString(object, 'o:kind', path);
  return SERVER_MEDIA_KINDS.find((kind) => kind === value) ?? null;
};

export const parseMedia: Parser<Media> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    ...parseEntityCore(object, path, brandId<'MediaId'>),
    ingester: asString(field(object, 'o:ingester', path), [...path, 'o:ingester']),
    renderer: asString(field(object, 'o:renderer', path), [...path, 'o:renderer']),
    kind: parseServerKind(object, path),
    item: nullableField(object, 'o:item', path, (value, fieldPath) =>
      asRef(value, fieldPath, brandId<'ItemId'>),
    ),
    source: nullableField(object, 'o:source', path, asString),
    mediaType: nullableField(object, 'o:media_type', path, asString),
    sha256: nullableField(object, 'o:sha256', path, asString),
    size: nullableField(object, 'o:size', path, asNumber),
    filename: nullableField(object, 'o:filename', path, asString),
    lang: nullableField(object, 'o:lang', path, asString),
    altText: nullableField(object, 'o:alt_text', path, asString),
    originalUrl: nullableField(object, 'o:original_url', path, asString),
    preview: parsePreview(object, path),
    access: parseAccess(object, path),
    thumbnailUrls: parseThumbnailUrls(field(object, 'o:thumbnail_urls', path), [
      ...path,
      'o:thumbnail_urls',
    ]),
  };
};

/**
 * The provider player URL `GET /api/romeka/media/{id}/player` names, or `null`.
 *
 * The route's whole answer is the URL or nothing, and only an `https` URL can
 * be framed: anything else degrades to `null`, which the stage reads as "frame
 * the server's render instead".
 */
export const parseMediaPlayer: Parser<string | null> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const url = nullableField(object, 'url', path, asString);
  if (url === null) {
    return null;
  }
  try {
    return new URL(url).protocol === 'https:' ? url : null;
  } catch {
    return null;
  }
};

/** Parses a media id out of a URL fragment such as `#/media/12`. */
export function mediaIdFromParam(text: string): MediaId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'MediaId'>(value);
}

export const getMedia = async (api: ApiClient, id: MediaId): Promise<Media> =>
  parseMedia(await api.requestJson(`/api/media/${String(id)}`));

export const getMediaPlayer = async (api: ApiClient, id: MediaId): Promise<string | null> =>
  parseMediaPlayer(await api.requestJson(`/api/romeka/media/${String(id)}/player`));

export const listMedia = (
  api: ApiClient,
  query: ResourceQuery = {},
): Promise<ApiPage<Media>> => api.searchPage('media', query, parseMedia);

export const listMediaForItem = (
  api: ApiClient,
  itemId: ItemId,
  query: ResourceQuery = {},
): Promise<ApiPage<Media>> => api.searchPage('media', { itemId, ...query }, parseMedia);

export const updateMedia = async (
  api: ApiClient,
  id: MediaId,
  write: MediaWrite,
): Promise<Media> =>
  parseMedia(await api.writeJson('PUT', `/api/media/${String(id)}`, encodeMedia(write)));

/**
 * `POST /api/romeka/media/{id}/derivatives/retry`: queue the media's access
 * copy again after its tool failed, and answer the media document with the
 * copy `pending`. The server refuses a reader who may not edit the media, and
 * a media nothing failed on.
 */
export const retryMediaAccess = async (api: ApiClient, id: MediaId): Promise<Media> =>
  parseMedia(
    await api.writeJson('POST', `/api/romeka/media/${String(id)}/derivatives/retry`, {}),
  );
