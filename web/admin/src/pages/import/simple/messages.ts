/**
 * The Simple screens' messages: their own `import-simple` namespace, the
 * `import` one for the words every import screen shares (`problem.<kind>`,
 * `status.*`, `words.*`), and `import-lists` for the review lists' words the
 * screens reuse (how many files a group has).
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { archiveImport } from '../../../i18n/namespaces/import';
import { importLists } from '../../../i18n/namespaces/import-lists';
import { importSimple } from '../../../i18n/namespaces/import-simple';
import type { ListsMessageKey } from '../lists/messages';
import type { ImportMessageKey } from '../messages';

export type SimpleMessageKey = KeyOf<typeof importSimple>;

export type SimpleTranslator = Translator<MessageKey | ImportMessageKey | ListsMessageKey | SimpleMessageKey>;

export const useSimpleText = (): SimpleTranslator => useTranslator(importSimple, archiveImport, importLists);
