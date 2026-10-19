/**
 * The vocabulary import and update messages, read through the `standards`
 * namespace (`src/i18n/namespaces/standards/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { standards } from '../../i18n/namespaces/standards';

/** A Standards namespace message id, which the import pages read. */
export type ImportMessageKey = KeyOf<typeof standards>;

/** A lookup into the Standards namespace. */
export type ImportText = (key: ImportMessageKey, params?: MessageParams) => string;

/** The import message lookup in the active locale. */
export const useImportText = (): ImportText => useTranslator(standards).t;
