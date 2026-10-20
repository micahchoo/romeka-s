/**
 * What the item picker knows apart from React: the props its callers pass, the
 * picks as one ordered list whatever the mode, the query a source and the
 * filters make, and the text a row and the peek show from a list response.
 */
import type { ResourceQuery } from '../../../../api/client';
import type { ClassId, Item, ItemId, ItemSetId, Value } from '../../../../api/types';

/** Several picks in the caller's order, which is the order the site shows them. */
export interface ManyPick {
  readonly mode: 'many';
  readonly picked: readonly ItemId[];
  readonly onChange: (picked: readonly ItemId[]) => void;
  /**
   * Whole item sets as picks too. When present, each set in the sources offers
   * "Pick the whole set" and the tray shows the picked sets.
   */
  readonly sets?: {
    readonly picked: readonly ItemSetId[];
    readonly onChange: (sets: readonly ItemSetId[]) => void;
  };
}

/** At most one pick, for a field that names one item. */
export interface OnePick {
  readonly mode: 'one';
  readonly picked: ItemId | null;
  readonly onChange: (picked: ItemId | null) => void;
}

export type ItemPickerProps = {
  readonly id: string;
  readonly label: string;
} & (ManyPick | OnePick);

/** The picks as a list, so the tray and the rows read one shape in both modes. */
export const pickedList = (props: ManyPick | OnePick): readonly ItemId[] =>
  props.mode === 'many' ? props.picked : props.picked === null ? [] : [props.picked];

/** The picks after a pick or an unpick of `item`. A single pick replaces the last one. */
export const toggled = (
  picked: readonly ItemId[],
  item: ItemId,
  mode: 'many' | 'one',
): readonly ItemId[] => {
  if (picked.includes(item)) {
    return picked.filter((entry) => entry !== item);
  }
  return mode === 'one' ? [item] : [...picked, item];
};

/** The list with the entry at `from` moved to `to`, or unchanged when either is outside it. */
export const moved = <T>(values: readonly T[], from: number, to: number): readonly T[] => {
  const entry = values[from];
  if (entry === undefined || to < 0 || to >= values.length || from === to) {
    return values;
  }
  const next = [...values];
  next.splice(from, 1);
  next.splice(to, 0, entry);
  return next;
};

/** Where the results come from: every item, or one item set. */
export type Source = { readonly kind: 'all' } | { readonly kind: 'set'; readonly id: ItemSetId };

export const ALL_ITEMS: Source = { kind: 'all' };

export interface Filters {
  readonly source: Source;
  /** The settled search text; empty searches nothing. */
  readonly search: string;
  readonly classId: ClassId | null;
  readonly hasMedia: boolean;
}

export const NO_FILTERS: Filters = { source: ALL_ITEMS, search: '', classId: null, hasMedia: false };

export const isFiltered = (filters: Filters): boolean =>
  filters.source.kind !== 'all' ||
  filters.search !== '' ||
  filters.classId !== null ||
  filters.hasMedia;

/** The `/api/items` query for the filters, without the page. The newest items come first. */
export const itemQuery = (filters: Filters): ResourceQuery => ({
  sortBy: 'id',
  sortOrder: 'desc',
  ...(filters.search === '' ? {} : { search: filters.search }),
  ...(filters.classId === null ? {} : { resourceClassId: filters.classId }),
  ...(filters.hasMedia ? { hasMedia: 1 } : {}),
  ...(filters.source.kind === 'set' ? { itemSetId: filters.source.id } : {}),
});

/** One value as the text a reader sees. */
export const valueText = (value: Value): string => {
  switch (value.kind) {
    case 'literal':
      return value.text;
    case 'uri':
      return value.label ?? value.uri;
    case 'resource':
      return value.displayTitle ?? '';
    case 'other':
      return '';
  }
};

/** The values a row shows beside the title, in this order, when the item has them. */
const KEY_TERMS = ['dcterms:creator', 'dcterms:date', 'dcterms:identifier'] as const;

/** The first value of each key term, for the row's one line of metadata. */
export const keyValues = (item: Item): readonly string[] =>
  KEY_TERMS.flatMap((term) => {
    const text = (item.values.get(term) ?? []).map(valueText).find((entry) => entry !== '');
    return text === undefined ? [] : [text];
  });

/** Every property the item has, with its label and its values' text, for the peek. */
export const valueRows = (
  item: Item,
): readonly { readonly term: string; readonly label: string; readonly texts: readonly string[] }[] =>
  [...item.values.entries()].flatMap(([term, values]) => {
    const texts = values.map(valueText).filter((text) => text !== '');
    const first = values[0];
    return first === undefined || texts.length === 0
      ? []
      : [{ term, label: first.propertyLabel, texts }];
  });

/** The item's class term from its JSON-LD types (`o:Item` comes first). */
export const classTerm = (item: Item): string | null =>
  item.types.find((type) => type !== 'o:Item') ?? null;
