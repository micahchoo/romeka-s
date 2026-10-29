/**
 * What the item picker knows apart from React: the props its callers pass, the
 * picks as one ordered list whatever the mode, the query a source and the
 * filters make, and the text a row and the peek show from a list response.
 */
import type { ResourceQuery } from '../../../../api/client';
import type {
  ClassId,
  Item,
  ItemId,
  ItemSetId,
  ServerMediaKind,
  Value,
} from '../../../../api/types';
import type { MediaKind } from '../../../../app/media';
import type { FieldAccepts } from '../../blockSchema';

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
  /**
   * What the field this picker fills shows, from the block's schema. A field
   * that takes an image starts narrowed to items holding one, and a field that
   * takes any media offers a kind to narrow by. Absent leaves the plain "Has
   * media" test.
   */
  readonly accepts?: FieldAccepts;
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

/**
 * What the picker asks of an item's media: nothing, any media, or a kind of
 * media. One value, because the three are one choice and separate flags would
 * have to be kept in agreement.
 */
export type MediaNarrowing =
  | { readonly kind: 'none' }
  | { readonly kind: 'media' }
  | { readonly kind: 'kinds'; readonly kinds: readonly ServerMediaKind[] };

export const NO_NARROWING: MediaNarrowing = { kind: 'none' };

/** One choice of the "Has" select: the admin's kind word and the wire kinds it narrows to. */
export interface HasChoice {
  /** The kinds an item must hold, empty for any media. */
  readonly kinds: readonly ServerMediaKind[];
  /** The admin's word for the choice, or `null` for any media. */
  readonly word: MediaKind | null;
}

const ANY_MEDIA: HasChoice = { kinds: [], word: null };

/** Every kind a builder can narrow by, in the order the select shows them. */
export const HAS_CHOICES: readonly HasChoice[] = [
  ANY_MEDIA,
  { kinds: ['local-image', 'iiif-image'], word: 'image' },
  { kinds: ['audio'], word: 'audio' },
  { kinds: ['video'], word: 'video' },
  { kinds: ['pdf'], word: 'pdf' },
  { kinds: ['model-3d'], word: 'model' },
  { kinds: ['embed'], word: 'embed' },
  { kinds: ['iiif-presentation'], word: 'manifest' },
];

/**
 * The wire kinds an item must hold for a field that takes `accepts`. The
 * narrowing picks the items; the field's own rule then decides which of their
 * media it can show, so a IIIF slot still lists an item whose images it cannot
 * zoom and says so on each tile.
 */
export const ACCEPT_KINDS: Readonly<Record<FieldAccepts, readonly ServerMediaKind[]>> = {
  image: ['local-image', 'iiif-image'],
  imageService: ['local-image', 'iiif-image'],
  any: [],
};

/** The narrowing a field that takes `accepts` starts from. */
export const narrowingFor = (accepts: FieldAccepts): MediaNarrowing => {
  const kinds = ACCEPT_KINDS[accepts];
  return kinds.length === 0 ? NO_NARROWING : { kind: 'kinds', kinds };
};

const sameKinds = (a: readonly ServerMediaKind[], b: readonly ServerMediaKind[]): boolean =>
  a.length === b.length && a.every((kind, at) => kind === b[at]);

/** The "Has" choice a narrowing shows, or any media when it narrows to no kinds. */
export const hasChoiceOf = (narrowing: MediaNarrowing): HasChoice =>
  narrowing.kind === 'kinds'
    ? (HAS_CHOICES.find((choice) => sameKinds(choice.kinds, narrowing.kinds)) ?? ANY_MEDIA)
    : ANY_MEDIA;

export interface Filters {
  readonly source: Source;
  /** The settled search text; empty searches nothing. */
  readonly search: string;
  readonly classId: ClassId | null;
  readonly narrowing: MediaNarrowing;
}

export const NO_FILTERS: Filters = {
  source: ALL_ITEMS,
  search: '',
  classId: null,
  narrowing: NO_NARROWING,
};

export const isFiltered = (filters: Filters): boolean =>
  filters.source.kind !== 'all' ||
  filters.search !== '' ||
  filters.classId !== null ||
  filters.narrowing.kind !== 'none';

/** The `/api/items` query for the filters, without the page. The newest items come first. */
export const itemQuery = (filters: Filters): ResourceQuery => ({
  sortBy: 'id',
  sortOrder: 'desc',
  ...(filters.search === '' ? {} : { search: filters.search }),
  ...(filters.classId === null ? {} : { resourceClassId: filters.classId }),
  ...(filters.narrowing.kind === 'media' ? { hasMedia: 1 } : {}),
  ...(filters.narrowing.kind === 'kinds' && filters.narrowing.kinds.length > 0
    ? { has_media_kind: filters.narrowing.kinds }
    : {}),
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
