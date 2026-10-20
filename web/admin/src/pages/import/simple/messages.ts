/**
 * The Simple screens' messages: their own `import-simple` namespace, and the
 * `import` one for the words every import screen shares (`problem.<kind>`,
 * `status.*`, `words.*`).
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { archiveImport } from '../../../i18n/namespaces/import';
import { importSimple } from '../../../i18n/namespaces/import-simple';
import type { ImportMessageKey } from '../messages';

export type SimpleMessageKey = KeyOf<typeof importSimple>;

export type SimpleTranslator = Translator<MessageKey | ImportMessageKey | SimpleMessageKey>;

export const useSimpleText = (): SimpleTranslator => useTranslator(importSimple, archiveImport);
