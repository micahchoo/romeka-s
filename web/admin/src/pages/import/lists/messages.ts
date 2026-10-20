/**
 * The review lists' messages: their own `import-lists` namespace over the
 * `import` one, which still holds `problem.<kind>` and the paged list's words.
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { archiveImport } from '../../../i18n/namespaces/import';
import { importLists } from '../../../i18n/namespaces/import-lists';

export type ListsMessageKey = KeyOf<typeof importLists>;

export type ListsTranslator = Translator<MessageKey | KeyOf<typeof archiveImport> | ListsMessageKey>;

export const useListsText = (): ListsTranslator => useTranslator(archiveImport, importLists);
