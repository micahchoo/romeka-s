/**
 * The Advanced screens' messages: their own namespace, `import-advanced`,
 * read with the `import` namespace for the words both modes share
 * (`problem.<kind>`, `status.*`, `words.*`).
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { importAdvanced } from '../../../i18n/namespaces/import-advanced';
import type { ImportMessageKey } from '../messages';
import { archiveImport } from '../../../i18n/namespaces/import';

export type AdvancedMessageKey = KeyOf<typeof importAdvanced>;

export type AdvancedTranslator = Translator<MessageKey | ImportMessageKey | AdvancedMessageKey>;

export const useAdvancedText = (): AdvancedTranslator => useTranslator(importAdvanced, archiveImport);
