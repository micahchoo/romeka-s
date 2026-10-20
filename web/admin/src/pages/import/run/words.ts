/**
 * Counts as words: "8 items, 13 files and 2 item sets". Each count is one the
 * server made (the check, the undo preview, the undo report); a kind with
 * none is left out of the sentence.
 */
import type { CheckCount } from '../../../api/import-sections';
import type { KindCounts, ResourceKind } from '../../../api/import-undo';
import type { RunTranslator } from './messages';

export type Counted = 'items' | 'files' | 'itemSets' | 'itemTypes' | 'fields' | 'problems';

export const countWords = ({ t, number }: RunTranslator, kind: Counted, count: number): string =>
  count === 1 ? t(`run.count.${kind}One`) : t(`run.count.${kind}`, { count: number(count) });

/** The non-zero counts joined as the locale joins a list, or `null` when every count is zero. */
export const listWords = (translator: RunTranslator, counts: readonly (readonly [Counted, number])[]): string | null => {
  const parts = counts.filter(([, count]) => count > 0).map(([kind, count]) => countWords(translator, kind, count));
  return parts.length === 0 ? null : new Intl.ListFormat(translator.locale, { type: 'conjunction' }).format(parts);
};

export const kindCountsWords = (translator: RunTranslator, counts: KindCounts): string | null =>
  listWords(translator, [
    ['items', counts.items],
    ['files', counts.media],
    ['itemSets', counts.itemSets],
    ['itemTypes', counts.templates],
    ['fields', counts.properties],
  ]);

/** The items a run writes: every item the check does not leave out. */
export const itemsWritten = (counts: readonly CheckCount[]): number =>
  counts.filter((count) => count.kind === 'item' && count.does !== 'leave').reduce((sum, count) => sum + count.count, 0);

export const resourceWords = ({ t, number }: RunTranslator, kind: ResourceKind, id: number): string =>
  t(`run.kind.${kind}`, { id: number(id) });

const ROUTE_OF: Readonly<Record<ResourceKind, string>> = {
  item: 'items',
  media: 'media',
  itemSet: 'item-sets',
  resourceTemplate: 'resource-templates',
  property: 'properties',
};

/** The admin page of one resource. */
export const resourceHref = (kind: ResourceKind, id: number): string => `#/${ROUTE_OF[kind]}/${String(id)}`;

/** "About 3 minutes left at this speed", from the seconds the server worked out from the rate so far. */
export const timeLeftWords = ({ t, number }: RunTranslator, secondsLeft: number | null): string => {
  if (secondsLeft === null) {
    return t('run.work.noTime');
  }
  if (secondsLeft < 60) {
    return t('run.work.seconds');
  }
  const minutes = Math.round(secondsLeft / 60);
  return minutes === 1 ? t('run.work.minute') : t('run.work.minutes', { count: number(minutes) });
};
