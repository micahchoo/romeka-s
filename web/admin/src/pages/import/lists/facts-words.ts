/**
 * A group of items in words, from its facts only (`docs/import-v2.md`: groups
 * are described by their facts, never by inferred meaning): how many files,
 * of which kinds, whether a list row joined, the parts the person ticked, and
 * the folders.
 */
import type { Bucket, MediaKind, ShapeFacts } from '../../../api/import-sections';
import type { ImportPlan } from '../../../api/import-plan';
import type { ListsMessageKey, ListsTranslator } from './messages';

export const BUCKET_WORDS: Readonly<Record<Bucket, ListsMessageKey>> = {
  '0': 'lists.facts.files0',
  '1': 'lists.facts.files1',
  '2': 'lists.facts.files2',
  '3': 'lists.facts.files3',
  '4-6': 'lists.facts.files4',
  '7+': 'lists.facts.files7',
};

const MEDIA_WORDS: Readonly<Record<MediaKind, ListsMessageKey>> = {
  image: 'lists.facts.image',
  pdf: 'lists.facts.pdf',
  audio: 'lists.facts.audio',
  video: 'lists.facts.video',
  other: 'lists.facts.other',
};

const quoted = (words: readonly string[]): string => words.map((word) => `“${word}”`).join(', ');

/** "2 files, all images, and a row in your list", for the sentence "N items have …". */
export const factsWords = (facts: ShapeFacts, plan: ImportPlan, t: ListsTranslator['t']): string => {
  const parts: string[] = [t(BUCKET_WORDS[facts.files])];
  if (facts.media.length === 1 && facts.media[0] !== undefined) {
    parts.push(t('lists.facts.allOf', { kind: t(MEDIA_WORDS[facts.media[0]]) }));
  } else if (facts.media.length > 1) {
    parts.push(facts.media.map((kind) => t(MEDIA_WORDS[kind])).join(t('lists.facts.and')));
  }
  if (facts.parts !== null && facts.parts.length > 0) {
    parts.push(t('lists.facts.parts', { parts: quoted(facts.parts) }));
  }
  if (facts.pages !== null && facts.pages.length > 0) {
    parts.push(t('lists.facts.pages', { count: String(facts.pages.length) }));
  }
  if (plan.grouping.pattern !== null && !facts.pattern) {
    parts.push(t('lists.facts.noFit'));
  }
  if (plan.sheets.length > 0) {
    parts.push(facts.row ? t('lists.facts.row') : t('lists.facts.noRow'));
  }
  return parts.join(', ');
};

/** "They are in the folder “letters”." */
export const foldersWords = (facts: ShapeFacts, t: ListsTranslator['t']): string => {
  const named = facts.folders.filter((folder) => folder !== '');
  if (named.length === 0) {
    return t('lists.facts.topFolder');
  }
  return named.length === 1
    ? t('lists.facts.folder', { folder: quoted(named) })
    : t('lists.facts.folders', { folders: quoted(named) });
};
