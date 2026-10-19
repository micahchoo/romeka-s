/**
 * The item type pages' messages, shared by the Item types and Fields routes,
 * read through the `item-types` namespace (`src/i18n/namespaces/item-types/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { itemTypes } from '../../i18n/namespaces/item-types';

export type { ItemTypesMessages } from '../../i18n/namespaces/item-types';

/** An item type page message id. */
export type ItemTypesMessageKey = KeyOf<typeof itemTypes>;

/** A lookup into the item type pages' table. */
export type ItemTypesText = (key: ItemTypesMessageKey, params?: MessageParams) => string;

/** The item type pages' message lookup in the active locale. */
export const useItemTypesText = (): ItemTypesText => useTranslator(itemTypes).t;
