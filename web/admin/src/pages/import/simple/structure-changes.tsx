/**
 * The changes strip under the Structure screen's tree: each click with what
 * it changed, or why it does not apply now, and a way to take it back. A
 * column put off the proposer's pick is a change too, read from the plan's
 * rules, with the pick as its way back. The strip shows only once the person
 * has a change, so it is a lazy chunk of its own, which the screen starts
 * loading as it opens. A click on a folder written
 * by hand waits while the folder stays so, with the way back beside it; a
 * split taken before it was written stays as its set level, and is kept.
 */
import type { ReactElement } from 'react';
import type { NotApplied } from '../../../api/import-proposal';
import type { CutEdit, Edit } from '../../../api/import-plan';
import { useDraft } from '../draft-context';
import { withColumnLevel } from './list-plan';
import { QUESTIONS, effectOf, nodeOf, questionOf } from './structure-edits';
import { FixInstances, fixAnswer, isFixing, staleLine, useFixText } from './structure-fixes';
import { columnChangesOf, folderName, grainWords, listName, pairWords, rollUpWords, setFromWords } from './structure-lines';
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
const editWords = (edit: Edit, words: Words, fix: ReturnType<typeof useFixText>): { readonly question: string; readonly answer: string } => {
  const { t } = words;
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
    case 'existing':
      return { question: t('structure.q.set'), answer: t('structure.set.existing', { id: String(edit.id) }) };
    case 'own':
      return { question: t('structure.hand.title'), answer: t('structure.own.mark') };
    case 'oneItem':
      return { question: t('structure.q.items'), answer: fixAnswer(edit, fix) };
    case 'rowIs':
    case 'describesNothing':
      return { question: t('structure.q.list'), answer: fixAnswer(edit, fix) };
  }
};

/** Where an edit acts, by the name of the node it names, else by the folder or list it names. */
const placeOf = (edit: Edit, tree: readonly TreeNode[], where: string, t: Words['t']): string => {
  const node = nodeOf(edit, tree);
  if (node?.kind === 'series' || node?.kind === 'own') {
    return folderName(node.node.scope.scope.folder, where);
  }
  if (node?.kind === 'kept') {
    return folderName(node.scope.folder, where);
  }
  if (node?.kind === 'every') {
    return t('structure.every');
  }
  switch (edit.edit) {
    case 'leaveOut':
    case 'keep':
    case 'items':
    case 'list':
    case 'own':
    case 'oneItem':
      return folderName(edit.scope.folder, where);
    case 'grain':
    case 'pair':
    case 'rowIs':
    case 'describesNothing':
      return listName(edit.list);
    case 'addSet':
    case 'dropSet':
    case 'title':
    case 'existing':
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
  const fix = useFixText();
  const columns = columnChangesOf(useDraft().plan, proposal);
  if (proposal.edits.length === 0 && columns.length === 0 && clicks.pending?.saved !== true) {
    return null;
  }
  return (
    <section className="structure-changes" aria-label={t('structure.count.changes')}>
      <h3>{t('structure.changes.title', { count: number(proposal.edits.length + columns.length) })}</h3>
      {clicks.pending?.saved === true ? <p role="status">{t('structure.change.saved')}</p> : null}
      <ul>
        {proposal.edits.map((result) => {
          const { question, answer } = editWords(result.edit, words, fix);
          const effect = effectOf(id, result.edit);
          // A click on a folder written by hand is not offered while the folder stays so: it waits, and comes back with the suggestion.
          const owner = result.reason === 'notOffered' ? nodeOf(result.edit, tree) : undefined;
          const waitsOn = owner?.kind === 'own' ? owner.node.scope.scope : null;
          // A split taken before the folder was written by hand stays as its set level: it is kept, not waiting.
          const kept = owner?.kind === 'kept';
          // A correction some of whose instances no longer hold still applies the others: it says how many, and each says why below.
          const stale = staleLine(result, fix);
          const after =
            kept
              ? t('structure.change.kept')
              : waitsOn !== null
              ? t('structure.change.waiting')
              : stale !== null
                ? stale
                : result.reason !== null
                ? t(REASONS[result.reason])
                : effect === undefined
                  ? null
                  : t('structure.change.effect', { before: number(effect.before), after: number(effect.after) });
          return (
            <li key={JSON.stringify(result.edit)} className={result.applied || kept ? undefined : 'is-waiting'}>
              <span>{t('structure.change.line', { where: placeOf(result.edit, tree, where, t), question, answer })}</span>
              {after === null ? null : <span className="structure-change-effect">{after}</span>}
              {waitsOn === null ? null : (
                <button
                  type="button"
                  disabled={!editable}
                  onClick={() => {
                    clicks.answer(QUESTIONS.own(waitsOn), 'back', null);
                  }}
                >
                  {t('structure.own.back')}
                </button>
              )}
              <button
                type="button"
                disabled={!editable}
                onClick={() => {
                  clicks.answer(questionOf(result.edit), 'back', null);
                }}
              >
                {t('structure.change.remove')}
              </button>
              {isFixing(result.edit) ? <FixInstances result={result} edit={result.edit} /> : null}
            </li>
          );
        })}
        {columns.map(({ node, column }) => {
          const back = column.rollUp.pick.chosen.value;
          const ref = { index: column.rollUp.index, header: column.rollUp.header };
          return (
            <li key={`column:${column.sheet}:${String(ref.index)}`}>
              <span>{t('structure.change.line', { where: folderName(node.scope.scope.folder, where), question: ref.header, answer: rollUpWords(column.level, t) })}</span>
              <button
                type="button"
                disabled={!editable}
                onClick={() => {
                  clicks.answer(QUESTIONS.column(column.list, ref.index), back, null, (plan) => withColumnLevel(plan, column.sheet, ref, back));
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

