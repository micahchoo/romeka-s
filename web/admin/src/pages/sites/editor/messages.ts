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

/**
 * The editor's counted messages: each noun's singular and plural ids. The
 * catalogue has no plural rules, so a count of exactly one reads the singular
 * message, the way every shipped locale writes "1 block".
 */
const COUNTED = {
  blocks: { one: 'canvas.blockCountOne', other: 'canvas.blockCount' },
  groupBlocks: { one: 'canvas.groupOne', other: 'canvas.group' },
  issues: { one: 'canvas.issuesOne', other: 'canvas.issues' },
  cardIssues: { one: 'canvas.cardIssuesOne', other: 'canvas.cardIssues' },
  pageIssues: { one: 'canvas.pageIssuesOne', other: 'canvas.pageIssues' },
  toolbarIssues: { one: 'editor.issueOne', other: 'editor.issueCount' },
  attachments: { one: 'canvas.attachmentsOne', other: 'canvas.attachments' },
} as const satisfies Readonly<Record<string, { one: EditorMessageKey; other: EditorMessageKey }>>;

/** The message for `count` of `noun`, with `count` and `params` filled in. */
export const counted = (
  t: (key: EditorMessageKey, params?: MessageParams) => string,
  noun: keyof typeof COUNTED,
  count: number,
  params: MessageParams = {},
): string => {
  const forms = COUNTED[noun];
  return t(count === 1 ? forms.one : forms.other, { ...params, count });
};
