/**
 * One decision of the plan in words, for the Changes list: its kind in the
 * contract's word and what it changes, naming files by path and rows by their
 * list and key.
 */
import type { Change, Decision, DecisionValue, EntryRef, FileEntryRef, MoveTo } from '../../../api/import-plan';
import type { AdvancedTranslator } from './messages';

/** How many of a `many` decision's entries are named. */
export const MANY_NAMED = 3;

export const entryWords = (entry: EntryRef): string => ('file' in entry ? entry.file.path : `${entry.row.sheet}: ${entry.row.key}`);

const fileWords = (entry: FileEntryRef): string => entry.file.path;

const moveWords = (to: MoveTo, t: AdvancedTranslator['t']): string => {
  if (to === 'own') {
    return t('adv.changes.move.own');
  }
  if (to === 'leave') {
    return t('adv.changes.move.leave');
  }
  return t('adv.changes.move.with', { file: fileWords(to.with) });
};

const valuesWords = (values: readonly DecisionValue[], t: AdvancedTranslator['t']): string =>
  values.length === 0 ? t('adv.changes.set.cleared') : values.map((value) => `“${value.text}”`).join(', ');

const changeWords = (change: Change, t: AdvancedTranslator['t']): string => {
  switch (change.kind) {
    case 'move':
      return moveWords(change.to, t);
    case 'leave':
      return t('adv.changes.leave');
    case 'set':
      return t('adv.changes.set', { slot: change.slot, values: valuesWords(change.values, t) });
    case 'match':
      return change.resource === null ? t('adv.changes.match.none') : t('adv.changes.match', { id: change.resource });
    case 'flag':
      return t('adv.changes.flag', { flag: change.flag, policy: change.policy });
  }
};

/** The decision's kind and what it does, e.g. `['move', 'a.jpg: with b.jpg']`. */
export const decisionWords = (decision: Decision, t: AdvancedTranslator['t']): readonly [string, string] => {
  switch (decision.kind) {
    case 'move':
      return ['move', t('adv.changes.on', { entry: fileWords(decision.file), change: moveWords(decision.to, t) })];
    case 'leave':
      return ['leave', t('adv.changes.on', { entry: entryWords(decision.entry), change: t('adv.changes.leave') })];
    case 'set':
      return ['set', t('adv.changes.on', { entry: entryWords(decision.entry), change: changeWords(decision, t) })];
    case 'match':
      return ['match', t('adv.changes.on', { entry: entryWords(decision.entry), change: changeWords(decision, t) })];
    case 'flag':
      return ['flag', t('adv.changes.on', { entry: entryWords(decision.entry), change: changeWords(decision, t) })];
    case 'pair':
      return ['pair', t('adv.changes.pair', { row: entryWords({ row: decision.row }), file: decision.file.path })];
    case 'many': {
      const named = decision.entries.slice(0, MANY_NAMED).map(entryWords).join(', ');
      const more = decision.entries.length - MANY_NAMED;
      return [
        'many',
        t('adv.changes.many', {
          count: decision.entries.length,
          entries: more > 0 ? t('adv.changes.manyMore', { named, more }) : named,
          change: changeWords(decision.change, t),
        }),
      ];
    }
  }
};
