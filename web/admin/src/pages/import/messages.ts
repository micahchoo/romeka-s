/**
 * The Import page's messages, read through the `import` namespace
 * (`src/i18n/namespaces/import/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../i18n';
import { archiveImport } from '../../i18n/namespaces/import';

export type ImportMessageKey = KeyOf<typeof archiveImport>;

export type ImportTranslator = Translator<MessageKey | ImportMessageKey>;

export const useImportText = (): ImportTranslator => useTranslator(archiveImport);
