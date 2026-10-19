/**
 * The page editor's field messages, read through the `editor` namespace
 * (`src/i18n/namespaces/editor/`): every block field's label, help line,
 * choice names and examples, the styling sections and the field controls' own
 * words. `src/generated/blocks.json` names only keys that table holds
 * (`parseBlockRegistry`).
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageParams } from '../../../i18n';
import { editor } from '../../../i18n/namespaces/editor';

export { en } from '../../../i18n/namespaces/editor';
export type { EditorMessages } from '../../../i18n/namespaces/editor';

/** One page editor message id. */
export type EditorMessageKey = KeyOf<typeof editor>;

/** Whether a string is a page editor message id. */
export const isEditorMessageKey = editor.has;

/** The editor's message lookup in the active locale. */
export const useEditorText = (): ((key: EditorMessageKey, params?: MessageParams) => string) =>
  useTranslator(editor).t;
