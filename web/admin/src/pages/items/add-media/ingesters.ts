/**
 * The words for the readers the server can name, and for the button that adds
 * one. A reader's word is a noun ("YouTube video"); a button's is the verb with
 * that noun ("Add YouTube video"), so the label states what the press does.
 */
import type { KeyOf } from '../../../i18n';
import type { Ingester } from '../../../api/staged-uploads';
import type { MediaKind } from '../../../app/media';
import type { mediaAdd } from '../../../i18n/namespaces/media-add';

export type MediaAddKey = KeyOf<typeof mediaAdd>;

/** The reader, as a noun. */
export const INGESTER_WORD: Readonly<Record<Ingester, MediaAddKey>> = {
  url: 'ingester.url',
  youtube: 'ingester.youtube',
  oembed: 'ingester.oembed',
  iiif: 'ingester.iiif',
  iiif_presentation: 'ingester.manifest',
  html: 'ingester.html',
};

/** The button's label for a reader: a verb and the same noun. */
export const SUBMIT_LABEL: Readonly<Record<Ingester, MediaAddKey>> = {
  url: 'submit.url',
  youtube: 'submit.youtube',
  oembed: 'submit.oembed',
  iiif: 'submit.iiif',
  iiif_presentation: 'submit.manifest',
  html: 'submit.html',
};

/** The plural word for a kind in the "takes" line. */
export const KINDS_WORD: Readonly<Record<MediaKind, MediaAddKey>> = {
  image: 'kinds.image',
  audio: 'kinds.audio',
  video: 'kinds.video',
  pdf: 'kinds.pdf',
  document: 'kinds.document',
  model: 'kinds.model',
  captions: 'kinds.captions',
  embed: 'kinds.embed',
  manifest: 'kinds.manifest',
  other: 'kinds.other',
};

/** The reading line's opening for a reader. */
export const READING_WORD: Readonly<Record<Ingester, MediaAddKey>> = {
  url: 'add.readingFile',
  youtube: 'add.readingYouTube',
  oembed: 'add.readingOembed',
  iiif: 'add.readingIiif',
  iiif_presentation: 'add.readingManifest',
  html: 'add.readingFile',
};
