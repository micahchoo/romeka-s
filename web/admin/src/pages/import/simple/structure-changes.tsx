/**
 * The changes strip under the Structure screen's tree: each click with what
 * it changed, or why it does not apply now, and a way to take it back. It
 * shows only once the person has a change, so it is a lazy chunk of its own,
 * which the screen starts loading as it opens.
 */
import type { ReactElement } from 'react';
import type { NotApplied } from '../../../api/import-proposal';
import type { CutEdit, Edit } from '../../../api/import-plan';
import { effectOf, nodeOf, questionOf } from './structure-edits';
import { folderName, grainWords, listName, pairWords, setFromWords } from './structure-lines';
import type { StructureMessageKey, TreeNode } from './structure-lines';
import { useScreen } from './structure-parts';
import type { Words } from './structure-parts';

const cutEditWords = (cut: CutEdit, t: Words['t']): string => {
  switch (cut.cut) {
    case 'whole':
      return t('structure.cut.whole');
    case 'places':
      return t('structure.cut.places', { places: String(cut.places) });
    case 'folder':
      return t('structure.cut.folder', { depth: String(cut.depth) });
  }
};

/** An edit as the question it answers and the answer, in the words of the inspector. */
const editWords = (edit: Edit, t: Words['t']): { readonly question: string; readonly answer: string } => {
  switch (edit.edit) {
    case 'leaveOut':
      return { question: t('structure.q.import'), answer: t('structure.import.no') };
    case 'keep':
      return { question: t('structure.q.import'), answer: t('structure.import.yes') };
    case 'items':
      return { question: t('structure.q.items'), answer: cutEditWords(edit.cut, t) };
    case 'list':
      return { question: t('structure.q.list'), answer: edit.list === null ? t('structure.list.no') : listName(edit.list) };
    case 'grain':
      return { question: t('structure.q.grain'), answer: grainWords(edit.describes, t) };
    case 'pair':
      return { question: t('structure.q.pair'), answer: pairWords(edit.rule, t) };
    case 'addSet':
      return { question: t('structure.q.set'), answer: setFromWords(edit.from, t) };
    case 'dropSet':
      return { question: t('structure.q.set'), answer: t('structure.set.none') };
    case 'title':
      return { question: t('structure.q.set'), answer: edit.title };
  }
};

/** Where an edit acts, by the name of the node it names, else by the folder or list it names. */
const placeOf = (edit: Edit, tree: readonly TreeNode[], where: string, t: Words['t']): string => {
  const node = nodeOf(edit, tree);
  if (node?.kind === 'series') {
    return folderName(node.node.scope.scope.folder, where);
  }
  if (node?.kind === 'every') {
    return t('structure.every');
  }
  switch (edit.edit) {
    case 'leaveOut':
    case 'keep':
    case 'items':
    case 'list':
      return folderName(edit.scope.folder, where);
    case 'grain':
    case 'pair':
      return listName(edit.list);
    case 'addSet':
    case 'dropSet':
    case 'title':
      return edit.from.from === 'column' ? listName(edit.from.list) : folderName(edit.from.scope.folder, where);
  }
};

const REASONS: Readonly<Record<NotApplied, StructureMessageKey>> = {
  gone: 'structure.change.gone',
  notOffered: 'structure.change.notOffered',
  replaced: 'structure.change.replaced',
};

/** The changes strip: each click with what it changed, or why it does not apply now, and a way to take it back. */
export const Changes = ({ tree, where, id }: { readonly tree: readonly TreeNode[]; readonly where: string; readonly id: Parameters<typeof effectOf>[0] }): ReactElement | null => {
  const { clicks, editable, proposal, words } = useScreen();
  const { t, number } = words;
  if (proposal.edits.length === 0 && clicks.pending?.saved !== true) {
    return null;
  }
  return (
    <section className="structure-changes" aria-label={t('structure.count.changes')}>
      <h3>{t('structure.changes.title', { count: number(proposal.edits.length) })}</h3>
      {clicks.pending?.saved === true ? <p role="status">{t('structure.change.saved')}</p> : null}
      <ul>
        {proposal.edits.map((result) => {
          const { question, answer } = editWords(result.edit, t);
          const effect = effectOf(id, result.edit);
          const after =
            result.reason !== null
              ? t(REASONS[result.reason])
              : effect === undefined
                ? null
                : t('structure.change.effect', { before: number(effect.before), after: number(effect.after) });
          return (
            <li key={JSON.stringify(result.edit)} className={result.applied ? undefined : 'is-waiting'}>
              <span>{t('structure.change.line', { where: placeOf(result.edit, tree, where, t), question, answer })}</span>
              {after === null ? null : <span className="structure-change-effect">{after}</span>}
              <button
                type="button"
                disabled={!editable}
                onClick={() => {
                  clicks.answer(questionOf(result.edit), 'back', null);
                }}
              >
                {t('structure.change.remove')}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
};

