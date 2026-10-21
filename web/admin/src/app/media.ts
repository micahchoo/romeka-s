/**
 * Derived values for the Media pages. Every function is pure, so the pages only
 * decide layout and never repeat the fallback rules.
 *
 * A media's kind is the one decision every screen reads: what a preview may
 * show, what an image picker offers, and what the placeholder says. It comes
 * from the renderer or ingester for a row with no file of its own, the media
 * type for a file, and a known extension for a type too generic to say (a
 * speech recording detected as `application/octet-stream`).
 */
import type { Media, ServerMediaKind, ThumbnailType } from '../api/types';

/** Order a browse cell prefers. Upstream shows the square thumbnail. */
const CELL_ORDER: readonly ThumbnailType[] = ['square', 'medium', 'large'];

/** Order the detail page prefers for a full-width image. */
const LARGE_ORDER: readonly ThumbnailType[] = ['large', 'medium', 'square'];

/** What a media is, as every admin screen names it. */
export type MediaKind =
  | 'image'
  | 'audio'
  | 'video'
  | 'pdf'
  | 'document'
  | 'model'
  | 'captions'
  | 'embed'
  | 'other';

/** Image types an `<img>` draws. A type outside this set keeps the placeholder. */
const SHOWABLE_IMAGE_TYPES: ReadonlySet<string> = new Set([
  'image/jpeg',
  'image/png',
  'image/gif',
  'image/webp',
  'image/avif',
  'image/svg+xml',
]);

/**
 * Renderers and ingesters whose media is not a file: a IIIF image is still an
 * image, while an HTML snippet and an outside player are embeds. A renderer
 * wins over the media type, because a YouTube poster frame is a JPEG the
 * renderer never shows in an `<img>`.
 */
const RENDERER_KINDS: Readonly<Record<string, MediaKind>> = {
  iiif: 'image',
  html: 'embed',
  oembed: 'embed',
  youtube: 'embed',
  iiif_presentation: 'embed',
};

const CAPTION_TYPES: ReadonlySet<string> = new Set([
  'text/vtt',
  'text/srt',
  'application/x-subrip',
  'application/ttml+xml',
]);

const MODEL_TYPES: ReadonlySet<string> = new Set([
  'model/gltf-binary',
  'model/gltf+json',
  'model/obj',
  'model/stl',
  'model/vnd.collada+xml',
  'model/vnd.usdz+zip',
]);

/** The types that are a document and belong to no family below. */
const DOCUMENT_TYPES: ReadonlySet<string> = new Set([
  'application/msword',
  'application/rtf',
  'application/epub+zip',
  'application/vnd.oasis.opendocument.text',
  'application/vnd.oasis.opendocument.spreadsheet',
  'application/vnd.oasis.opendocument.presentation',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  'application/vnd.openxmlformats-officedocument.presentationml.presentation',
]);

/** The type families, first match wins. Captions and models are read before these. */
const TYPE_FAMILIES: readonly (readonly [string, MediaKind])[] = [
  ['image/', 'image'],
  ['audio/', 'audio'],
  ['video/', 'video'],
  ['text/', 'document'],
];

/** Extensions a type too generic to say (octet-stream, text/plain) still names. */
const KIND_EXTENSIONS: Readonly<Record<string, MediaKind>> = {
  avif: 'image',
  bmp: 'image',
  gif: 'image',
  heic: 'image',
  jp2: 'image',
  jpeg: 'image',
  jpg: 'image',
  png: 'image',
  svg: 'image',
  tif: 'image',
  tiff: 'image',
  webp: 'image',
  aac: 'audio',
  aif: 'audio',
  aiff: 'audio',
  flac: 'audio',
  m4a: 'audio',
  mid: 'audio',
  midi: 'audio',
  mp3: 'audio',
  oga: 'audio',
  ogg: 'audio',
  opus: 'audio',
  wav: 'audio',
  wma: 'audio',
  avi: 'video',
  flv: 'video',
  m4v: 'video',
  mkv: 'video',
  mov: 'video',
  mp4: 'video',
  mpeg: 'video',
  mpg: 'video',
  ogv: 'video',
  qt: 'video',
  webm: 'video',
  wmv: 'video',
  pdf: 'pdf',
  dfxp: 'captions',
  srt: 'captions',
  ttml: 'captions',
  vtt: 'captions',
  '3mf': 'model',
  dae: 'model',
  fbx: 'model',
  glb: 'model',
  gltf: 'model',
  obj: 'model',
  ply: 'model',
  stl: 'model',
  usd: 'model',
  usda: 'model',
  usdc: 'model',
  usdz: 'model',
  csv: 'document',
  doc: 'document',
  docx: 'document',
  epub: 'document',
  htm: 'document',
  html: 'document',
  json: 'document',
  md: 'document',
  odt: 'document',
  odp: 'document',
  ods: 'document',
  ppt: 'document',
  pptx: 'document',
  rtf: 'document',
  tex: 'document',
  txt: 'document',
  xls: 'document',
  xlsx: 'document',
  xml: 'document',
};

const nonEmpty = (text: string | null): string | null =>
  text === null || text === '' ? null : text;

/** The media type without its parameters, lowercased. */
const bareType = (media: Media): string | null =>
  media.mediaType === null ? null : (media.mediaType.split(';')[0]?.trim().toLowerCase() ?? null);

/** The lowercase extension of a name, URL or stored filename. */
const extensionOf = (name: string): string | null => {
  const path = name.split(/[?#]/)[0] ?? '';
  const base = path.slice(path.lastIndexOf('/') + 1);
  const dot = base.lastIndexOf('.');
  if (dot <= 0 || dot === base.length - 1) {
    return null;
  }
  const extension = base.slice(dot + 1).toLowerCase();
  return /^[a-z0-9]{1,12}$/.test(extension) ? extension : null;
};

/**
 * The file's extension, upper case, from the stored name then the source. Only
 * a file renderer names a file: an outside player's source is a URL of a page
 * or a manifest, and its extension would name the wrong thing.
 */
export const fileExtension = (media: Media): string | null => {
  if (media.renderer !== 'file') {
    return null;
  }
  const name = nonEmpty(media.filename) ?? nonEmpty(media.source);
  const extension = name === null ? null : extensionOf(name);
  return extension === null ? null : extension.toUpperCase();
};

/**
 * The server's kind in the admin's words, one row per `MediaKind` variant. A
 * variant the admin has no word for keeps the nearest one: an IIIF manifest is
 * an embed until the kind-words unit gives it its own word. `other-file` has no
 * word of its own, because the server's catch-all covers both a document and a
 * plain file, and its type is what tells them apart.
 */
const SERVER_KIND_WORDS: Readonly<Record<ServerMediaKind, MediaKind | 'by-type'>> = {
  'local-image': 'image',
  'iiif-image': 'image',
  audio: 'audio',
  video: 'video',
  pdf: 'pdf',
  'model-3d': 'model',
  captions: 'captions',
  embed: 'embed',
  'iiif-presentation': 'embed',
  'other-file': 'by-type',
};

/** The admin's word for the server's `other-file`: a document, else a file. */
const documentOrOther = (media: Media): MediaKind => {
  const type = bareType(media);
  const extension = fileExtension(media)?.toLowerCase() ?? null;
  if (type !== null && DOCUMENT_TYPES.has(type)) {
    return 'document';
  }
  return extension !== null && KIND_EXTENSIONS[extension] === 'document' ? 'document' : 'other';
};

/** What the media is, from the admin's own table. The fallback for an old server. */
const kindFromTable = (media: Media): MediaKind => {
  const rendered = RENDERER_KINDS[media.renderer] ?? RENDERER_KINDS[media.ingester];
  if (rendered !== undefined) {
    return rendered;
  }
  const type = bareType(media);
  const extension = fileExtension(media)?.toLowerCase() ?? null;
  const byExtension = extension === null ? undefined : KIND_EXTENSIONS[extension];
  if (type === 'application/pdf' || byExtension === 'pdf') {
    return 'pdf';
  }
  if (CAPTION_TYPES.has(type ?? '') || byExtension === 'captions') {
    return 'captions';
  }
  if (MODEL_TYPES.has(type ?? '') || byExtension === 'model') {
    return 'model';
  }
  const family = TYPE_FAMILIES.find(([prefix]) => type?.startsWith(prefix) === true);
  if (family !== undefined) {
    return family[1];
  }
  if (type !== null && DOCUMENT_TYPES.has(type)) {
    return 'document';
  }
  return byExtension ?? 'other';
};

/**
 * What the media is. The one read every preview and picker derives from. The
 * server's kind wins when the document carries it; the table above is the
 * fallback for a server that predates `o:kind`.
 */
export const mediaKind = (media: Media): MediaKind => {
  if (media.kind === null) {
    return kindFromTable(media);
  }
  const word = SERVER_KIND_WORDS[media.kind];
  return word === 'by-type' ? documentOrOther(media) : word;
};

/**
 * `MediaRepresentation::displayTitle` falls back to the media source. This
 * collection has no media titles, so the source or the filename is what the
 * row is called. `null` when the row has none, so the caller supplies the
 * catalogue's placeholder.
 */
export const mediaName = (media: Media): string | null =>
  nonEmpty(media.title) ?? nonEmpty(media.source) ?? nonEmpty(media.filename);

const thumbnailUrl = (media: Media, order: readonly ThumbnailType[]): string | null => {
  for (const type of order) {
    const url = nonEmpty(media.thumbnailUrls[type] ?? null);
    if (url !== null) {
      return url;
    }
  }
  return null;
};

/**
 * The original file, when the browser draws it in an `<img>`. A PDF, a video or
 * an audio file in an image element is a broken image, so those answer `null`
 * and the caller shows the kind's placeholder.
 */
const originalImageUrl = (media: Media): string | null => {
  const type = bareType(media);
  return type !== null && SHOWABLE_IMAGE_TYPES.has(type) ? nonEmpty(media.originalUrl) : null;
};

/**
 * The image a browse cell shows: a thumbnail when the port produced one, the
 * original otherwise when it is an image the browser draws. `o:thumbnail_urls`
 * is empty on every row of this collection, so the original is the common case.
 */
export const previewImageUrl = (media: Media): string | null =>
  thumbnailUrl(media, CELL_ORDER) ?? originalImageUrl(media);

/** The image the detail page shows at full width. */
export const largeImageUrl = (media: Media): string | null =>
  thumbnailUrl(media, LARGE_ORDER) ?? originalImageUrl(media);

/**
 * The image a media contributes to an image-only picker. A thumbnail of an
 * audio or video poster is not an image choice, so the kind decides first.
 */
export const imagePreviewUrl = (media: Media): string | null =>
  mediaKind(media) === 'image' ? previewImageUrl(media) : null;

/** The stored original file, when the row still has one. */
export const originalFileUrl = (media: Media): string | null => nonEmpty(media.originalUrl);

/** The file a media preview links to: the original, else the best thumbnail. */
export const previewLinkUrl = (media: Media): string | null =>
  nonEmpty(media.originalUrl) ?? thumbnailUrl(media, CELL_ORDER);
