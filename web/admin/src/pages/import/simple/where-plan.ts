/**
 * The plan edits "Where they go" makes: the item sets and the item type
 * rule with its value map. A column a list sends to the item sets stays
 * first in the item sets rule, so a list's value wins over the folder's
 * (`docs/import-v2.md`: a filled list's values win by default). One set for
 * everything is a set named by its title, found or made; a set picked from
 * the install is named by its id. Each is a free set of its own
 * (`../series.ts`).
 */
import type { ImportPlan, LevelId, SetById, SetByTitle, SetSource, SlotRule, SourceRef } from '../../../api/import-plan';
import { listOf, seriesOf, setsOf, setsRuleOf, withSeries, withSets, withSetsRule } from '../series';

export type SetChoice = 'leaf' | 'top' | 'one' | 'old' | 'column' | 'none';

const ruleOf = (plan: ImportPlan, level: LevelId, slot: 'template'): SlotRule => seriesOf(plan, level).slots.item[slot] ?? { sources: [], combine: 'first' };

const isEmpty = (rule: SlotRule): boolean => rule.sources.length === 0 && (rule.map === undefined || Object.keys(rule.map).length === 0);

const withRule = (plan: ImportPlan, level: LevelId, slot: 'template', rule: SlotRule): ImportPlan =>
  withSeries(plan, level, (series) => {
    const item = Object.fromEntries(Object.entries(series.slots.item).filter(([name]) => name !== slot));
    return { ...series, slots: { ...series.slots, item: isEmpty(rule) ? item : { ...item, [slot]: rule } } };
  });

const own = (rule: SlotRule): SourceRef | undefined => rule.sources.find((source) => source.from !== 'column');

const isNamed = (set: SetSource): set is SetById | SetByTitle => !('each' in set);

/** The folder name a new item set for everything takes. */
export const folderName = (plan: ImportPlan): string => {
  const folder = plan.sources.find((source) => source.kind === 'folder');
  if (folder?.kind !== 'folder') {
    return '';
  }
  return folder.path === '' ? folder.root : (folder.path.split('/').at(-1) ?? folder.path);
};

/** The header of the first list column that names item sets, if one does. */
export const setColumnOf = (plan: ImportPlan): string | null => {
  const source = setsOf(plan)
    .flatMap((set) => ('each' in set ? set.each.sources : []))
    .find((one) => one.from === 'column');
  return source?.from === 'column' ? source.column.header : null;
};

export const setChoiceOf = (plan: ImportPlan): { readonly choice: SetChoice; readonly set: string | null } => {
  const named = setsOf(plan).find(isNamed);
  if (named !== undefined) {
    return 'id' in named ? { choice: 'old', set: String(named.id) } : { choice: 'one', set: named.title };
  }
  const source = own(setsRuleOf(plan) ?? { sources: [], combine: 'first' });
  if (source === undefined) {
    return setColumnOf(plan) === null ? { choice: 'none', set: null } : { choice: 'column', set: null };
  }
  if (source.from === 'folder') {
    return { choice: source.level === 'path' ? 'leaf' : 'top', set: null };
  }
  if (source.from === 'constant') {
    // A draft from before v3 kept its one set as a constant, read as v2 read it.
    return /^\d+$/.test(source.value) ? { choice: 'old', set: source.value } : { choice: 'one', set: source.value };
  }
  return { choice: 'none', set: null };
};

/** The set every item joins for `choice`: a folder in the item sets rule, or a set of its own by title or id. */
export const withSetChoice = (plan: ImportPlan, choice: SetChoice, set: string | null): ImportPlan => {
  if (choice === 'none') {
    return withSets(plan, () => []);
  }
  const rule = setsRuleOf(plan) ?? { sources: [], combine: 'first' };
  const folder: readonly SourceRef[] = choice === 'leaf' ? [{ from: 'folder', level: 'path' }] : choice === 'top' ? [{ from: 'folder', level: 1 }] : [];
  const sources = [...rule.sources.filter((source) => source.from === 'column'), ...folder];
  const kept = withSetsRule(
    withSets(plan, (sets) => sets.filter((one) => !isNamed(one))),
    isEmpty({ ...rule, sources }) ? undefined : { ...rule, sources },
  );
  const named: SetSource | null =
    choice === 'one'
      ? { title: set ?? folderName(plan), description: null }
      : choice === 'old' && set !== null && /^\d+$/.test(set)
        ? { id: Number(set) }
        : null;
  return named === null ? kept : withSets(kept, (sets) => [...sets, named]);
};

/** Where the item type of a series comes from: nowhere, a list's column (`sheet` and index), or the top folder. */
export type TypeFrom = { readonly from: 'none' } | { readonly from: 'top' } | { readonly from: 'column'; readonly sheet: string; readonly index: number };

export const typeFromOf = (plan: ImportPlan, level: LevelId): TypeFrom => {
  const first = ruleOf(plan, level, 'template').sources[0];
  if (first?.from === 'column') {
    return { from: 'column', sheet: first.sheet, index: first.column.index };
  }
  if (first?.from === 'folder') {
    return { from: 'top' };
  }
  return { from: 'none' };
};

export const withTypeFrom = (plan: ImportPlan, level: LevelId, from: TypeFrom): ImportPlan => {
  const rule = ruleOf(plan, level, 'template');
  if (from.from === 'none') {
    return withRule(plan, level, 'template', { ...rule, sources: [] });
  }
  if (from.from === 'top') {
    return withRule(plan, level, 'template', { ...rule, sources: [{ from: 'folder', level: 1 }] });
  }
  const column = listOf(plan, from.sheet)?.columns.find((one) => one.column.index === from.index)?.column;
  return column === undefined ? plan : withRule(plan, level, 'template', { ...rule, sources: [{ from: 'column', sheet: from.sheet, column }] });
};
