/**
 * The one element a media's stage shows, decided once from the media's kind.
 *
 * The page used to show one `<img>` for every kind, which is a false
 * affordance: it reads as "here is the media" and shows a broken picture for
 * everything that is not one. The admin's kind table (`app/media.ts`) already
 * carries the decision, so this module only maps a kind to the element its
 * browser control needs, in one exhaustive switch a new kind cannot skip.
 *
 * A kind whose file the admin cannot use falls back to the file card, which
 * names the file and lets the editor take it. A media with no file and no link
 * says so; it never renders an empty box.
 */
import type { Media } from '../../../api/types';
import type { MediaKind } from '../../../app/media';
import {
  fileExtension,
  largeImageUrl,
  mediaKind,
  mediaName,
  originalFileUrl,
} from '../../../app/media';

/** The facts a file card states, and the link that takes the file. */
export interface FileFacts {
  readonly name: string | null;
  readonly kind: MediaKind;
  readonly extension: string | null;
  readonly size: number | null;
  /** The stored original; `null` when the row kept none. */
  readonly url: string | null;
  /** The row's source, when it is a web address the card may link. */
  readonly sourceUrl: string | null;
}

/**
 * What the stage draws. `element` names the browser control, not the media
 * kind, because the two where the media is shown are the two an editor acts
 * on: a player, a page, a picture, a file.
 */
export type StageView =
  | {
      readonly element: 'image';
      readonly url: string | null;
      readonly alt: string;
      readonly svg: boolean;
      readonly file: FileFacts;
    }
  | { readonly element: 'audio'; readonly url: string; readonly file: FileFacts }
  | {
      readonly element: 'video';
      readonly url: string;
      readonly poster: string | null;
      readonly file: FileFacts;
    }
  | {
      readonly element: 'pdf';
      readonly pageUrl: string | null;
      readonly fileUrl: string;
      readonly file: FileFacts;
    }
  | { readonly element: 'text'; readonly textUrl: string; readonly file: FileFacts }
  | {
      readonly element: 'outside';
      readonly url: string;
      readonly host: string;
      readonly file: FileFacts;
    }
  | {
      /**
       * An oEmbed or HTML media, which only the server may render. The stage
       * puts the server's render in a sandboxed frame on the editor's press;
       * `host` is `null` for an HTML snippet that names no page.
       */
      readonly element: 'frame';
      readonly url: string | null;
      readonly host: string | null;
      readonly file: FileFacts;
    }
  | {
      readonly element: 'manifest';
      readonly url: string;
      readonly host: string;
      readonly viewerUrl: string;
      readonly file: FileFacts;
    }
  | { readonly element: 'file'; readonly file: FileFacts }
  | { readonly element: 'none' };

/** The host a link contacts, named before the editor presses it. */
const hostOf = (url: string): string => {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
};

const httpUrl = (value: string | null): string | null =>
  value !== null && /^https?:\/\//i.test(value) ? value : null;

const factsOf = (media: Media): FileFacts => ({
  name: mediaName(media),
  kind: mediaKind(media),
  extension: fileExtension(media),
  size: media.size,
  url: originalFileUrl(media),
  sourceUrl: httpUrl(media.source),
});

/**
 * The stage for one media. The media kind decides the element; a missing file
 * only ever removes a control, it never leaves the stage blank.
 */
export const stageView = (media: Media): StageView => {
  const kind = mediaKind(media);
  const file = factsOf(media);
  const original = file.url;
  switch (kind) {
    case 'image': {
      const svg = media.mediaType === 'image/svg+xml';
      return {
        element: 'image',
        // A drawing stays its own vector original: a raster derivative would
        // lose the detail that makes it worth zooming.
        url: svg ? (original ?? largeImageUrl(media)) : largeImageUrl(media),
        alt: media.altText ?? file.name ?? '',
        svg,
        file,
      };
    }
    case 'audio':
      return original === null ? { element: 'file', file } : { element: 'audio', url: original, file };
    case 'video':
      return original === null
        ? { element: 'file', file }
        : { element: 'video', url: original, poster: largeImageUrl(media), file };
    case 'pdf':
      return original === null
        ? { element: 'file', file }
        : { element: 'pdf', pageUrl: largeImageUrl(media), fileUrl: original, file };
    case 'captions':
      return original === null ? { element: 'file', file } : { element: 'text', textUrl: original, file };
    case 'model':
      // The 3D viewer belongs to the public page; the stage states the file.
      return { element: 'file', file };
    case 'manifest': {
      const source = httpUrl(media.source);
      return source === null
        ? { element: 'file', file }
        : {
            element: 'manifest',
            url: source,
            host: hostOf(source),
            viewerUrl: `/iiif-viewer?url=${encodeURIComponent(source)}`,
            file,
          };
    }
    case 'embed': {
      const source = httpUrl(media.source);
      // Only oEmbed and HTML markup needs the server's render: the admin must
      // not decide what provider HTML is safe. A YouTube link is a player the
      // admin can name and open, so it keeps the plain card.
      if (media.renderer === 'oembed' || media.renderer === 'html') {
        return {
          element: 'frame',
          url: source,
          host: source === null ? null : hostOf(source),
          file,
        };
      }
      return source === null
        ? { element: 'none' }
        : { element: 'outside', url: source, host: hostOf(source), file };
    }
    case 'document':
    case 'other':
      return original === null && file.sourceUrl === null ? { element: 'none' } : { element: 'file', file };
  }
};
