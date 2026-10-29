/**
 * The page editor's field messages, read through the `editor` namespace
 * (`src/i18n/namespaces/editor/`) and the content blocks' own
 * (`src/i18n/namespaces/content-blocks/`): every block field's label, help
 * line, choice names and examples, the styling sections and the field
 * controls' own words. `src/generated/blocks.json` names only keys those
 * tables hold (`parseBlockRegistry`).
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageParams } from '../../../i18n';
import { contentBlocks, en as contentEn } from '../../../i18n/namespaces/content-blocks';
import type { ContentBlockMessages } from '../../../i18n/namespaces/content-blocks';
import { editor, en as editorEn } from '../../../i18n/namespaces/editor';
import type { EditorMessages as EditorTable } from '../../../i18n/namespaces/editor';

/** The two tables the page editor reads its messages from, for a
 * `useTranslator` call that also needs another namespace. */
export const EDITOR_NAMESPACES = [editor, contentBlocks] as const;

/** Both tables' English. */
export const en = { ...editorEn, ...contentEn };

/** Both tables in one locale. */
export type EditorMessages = EditorTable & ContentBlockMessages;

/** One page editor message id. */
export type EditorMessageKey = KeyOf<typeof editor> | KeyOf<typeof contentBlocks>;

/** Whether a string is a page editor message id. */
export const isEditorMessageKey = (key: string): key is EditorMessageKey =>
  editor.has(key) || contentBlocks.has(key);

/** The editor's message lookup in the active locale. */
export const useEditorText = (): ((key: EditorMessageKey, params?: MessageParams) => string) =>
  useTranslator(...EDITOR_NAMESPACES).t;

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
  pages: { one: 'canvas.pagesOne', other: 'canvas.pages' },
  blocksAdded: { one: 'history.addedOne', other: 'history.added' },
  blocksRemoved: { one: 'history.removedOne', other: 'history.removed' },
  blocksChanged: { one: 'history.changedOne', other: 'history.changed' },
  blocksChangedNamed: { one: 'history.changedNamedOne', other: 'history.changedNamed' },
  blocksMoved: { one: 'history.movedOne', other: 'history.moved' },
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
