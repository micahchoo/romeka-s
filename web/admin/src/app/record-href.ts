/**
 * Where a record opens in the admin: an item its page, an item set its place on the Items
 * screen, a media its page.
 */
import type { RecordSummary } from '../api/places';
import { setPlaceHref } from '../pages/item-sets/facts';

export const recordHref = (record: RecordSummary): string => {
  switch (record.kind) {
    case 'items':
      return `#/items/${String(record.id)}`;
    case 'item_sets':
      return setPlaceHref(record.id);
    case 'media':
      return `#/media/${String(record.id)}`;
  }
};
