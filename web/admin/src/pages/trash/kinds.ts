/**
 * What differs between the kinds of record a page moves to the trash alone: the confirm's words,
 * the screen that offers the Undo, and where the record is once it is back. Everything else in
 * the confirm and the notice is the same for an item, a media and a set.
 */
import type { TrashOf } from '../../api/bulk';
import type { KeyOf } from '../../i18n';
import type { bulkDelete } from '../../i18n/namespaces/bulk-delete';
import { setPlaceHref } from '../item-sets/facts';
import type { TrashTarget } from './slot';

type Key = KeyOf<typeof bulkDelete>;

export interface TrashKind {
  readonly checking: Key;
  readonly cannotCheck: Key;
  readonly denied: Key;
  /** How long the trash keeps it, and where its Undo is. */
  readonly trash: Key;
  /** The screen the delete opens, which offers the Undo. */
  readonly listHref: string;
}

export const TRASH_KINDS: Readonly<Record<TrashOf, TrashKind>> = {
  items: {
    checking: 'bulkDelete.item.checking',
    cannotCheck: 'bulkDelete.item.cannotCheck',
    denied: 'bulkDelete.item.denied',
    trash: 'bulkDelete.item.trash',
    listHref: '#/items',
  },
  media: {
    checking: 'bulkDelete.media.checking',
    cannotCheck: 'bulkDelete.media.cannotCheck',
    denied: 'bulkDelete.media.denied',
    trash: 'bulkDelete.media.trash',
    listHref: '#/media',
  },
  item_sets: {
    checking: 'bulkDelete.set.checking',
    cannotCheck: 'bulkDelete.set.cannotCheck',
    denied: 'bulkDelete.set.denied',
    trash: 'bulkDelete.set.trash',
    listHref: '#/item-sets',
  },
};

/** The record's own page, where the notice links once it is back. */
export const recordHref = (target: TrashTarget): string => {
  switch (target.of) {
    case 'items':
      return `#/items/${String(target.id)}`;
    case 'media':
      return `#/media/${String(target.id)}`;
    case 'item_sets':
      return setPlaceHref(target.id);
  }
};
