/**
 * The item page's line for the list it was opened from: "12 of 382 in Box 4",
 * with links to the items before and after it. The rows come from the list's
 * own query, as the caller, so the number is the list's total; an item no
 * longer near its position shows no line at all.
 */
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { brandId } from '../../api/parse';
import { placesApi } from '../../api/places';
import type { Item, ItemId } from '../../api/types';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { chosenId } from './browse';
import { itemPageHref, locate, WINDOW, windowPages, windowQuery } from './walk';
import type { Located, Walk } from './walk';
import './walk-line.css';

/** Where the list lives: a set's place or a record's links place, by the record's title, or the whole Items list. */
type Place = { readonly kind: 'items' } | { readonly kind: 'record'; readonly title: string | null };

const placeOf = async (walk: Walk): Promise<Place> => {
  const { itemSetId, links } = walk.browse.filters;
  const target = chosenId(itemSetId) ?? links?.target ?? null;
  if (target === null) {
    return { kind: 'items' };
  }
  const record = await placesApi.record(brandId<'ResourceId'>(target));
  return { kind: 'record', title: record.title };
};

export interface WalkFound extends Located {
  readonly walk: Walk;
  readonly total: number;
  readonly place: Place;
}

const loadWalk = async (itemId: ItemId, walk: Walk): Promise<WalkFound | null> => {
  const [place, pages] = await Promise.all([
    placeOf(walk),
    Promise.all(windowPages(walk.at).map((page) => api.listItems(windowQuery(walk, page)))),
  ]);
  const totals = new Set(pages.map((page) => page.total));
  // Two reads that disagree on the total saw the list change between them.
  if (totals.size !== 1) {
    return null;
  }
  const rows = new Map<number, Item>(
    pages.flatMap((page) => page.items.map((item, index) => [(page.page - 1) * WINDOW + index + 1, item] as const)),
  );
  const located = locate(rows, itemId, walk.at);
  return located === null ? null : { ...located, walk, total: pages[0]?.total ?? 0, place };
};

/**
 * The walk for this item, or null while it loads, when the address names none,
 * or when the item cannot be found near its position. A refused read is the
 * same null: the line is a convenience, and the page above it still stands.
 */
export const useWalk = (itemId: ItemId, walk: Walk | null): WalkFound | null => {
  const key = walk === null ? null : itemPageHref(itemId, { walk, edit: false });
  const state = useAsync(
    () => (walk === null ? Promise.resolve(null) : loadWalk(itemId, walk)),
    [key],
  );
  return state.status === 'ready' ? state.data : null;
};

const titleOf = (item: Item, untitled: string): string => item.title ?? untitled;

export const WalkLine = ({ found }: { readonly found: WalkFound }): ReactElement => {
  const { t, number } = useTranslator(common, resources);
  const { walk, at, total, previous, next, place } = found;
  const untitled = t('common.untitled');
  const placeName = place.kind === 'items' ? t('page.items') : (place.title ?? untitled);
  const hrefAt = (item: Item, position: number): string =>
    itemPageHref(item.id, { walk: { browse: walk.browse, at: position }, edit: false });
  return (
    <nav className="item-walk" aria-label={t('walk.label')}>
      {previous === null ? null : (
        <a className="item-walk-previous" rel="prev" href={hrefAt(previous, at - 1)}>
          {t('walk.previous', { title: titleOf(previous, untitled) })}
        </a>
      )}
      <p className="item-walk-position">
        {t('walk.position', { at: number(at), total: number(total), place: placeName })}
      </p>
      {next === null ? null : (
        <a className="item-walk-next" rel="next" href={hrefAt(next, at + 1)}>
          {t('walk.next', { title: titleOf(next, untitled) })}
        </a>
      )}
    </nav>
  );
};

/** Where "Save and next" goes: the next item's page with its edit form open. */
export const saveNextHref = (found: WalkFound): string | null =>
  found.next === null
    ? null
    : itemPageHref(found.next.id, { walk: { browse: found.walk.browse, at: found.at + 1 }, edit: true });
