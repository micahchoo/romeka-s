/**
 * The run screen reads its own namespace (`import-run`) and the import's
 * shared words (`import`: the status names, the copy button).
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { archiveImport } from '../../../i18n/namespaces/import';
import { importRun } from '../../../i18n/namespaces/import-run';
import type { ImportMessageKey } from '../messages';

export type RunMessageKey = KeyOf<typeof importRun>;

export type RunTranslator = Translator<MessageKey | ImportMessageKey | RunMessageKey>;

export const useRunText = (): RunTranslator => useTranslator(archiveImport, importRun);
