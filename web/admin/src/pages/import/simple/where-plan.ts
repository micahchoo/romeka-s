/**
 * The plan edits "Where they go" makes: the item set rule and the item type
 * rule with its value map. A column a list sends to the item set stays first,
 * so a list's value wins over the folder's (`docs/import-v2.md`: a filled
 * list's values win by default).
 */
import type { ImportPlan, SlotRule, SourceRef } from '../../../api/import-plan';
import { fold } from './list-plan';

export type SetChoice = 'leaf' | 'top' | 'one' | 'old' | 'column' | 'none';

const ruleOf = (plan: ImportPlan, slot: 'sets' | 'template'): SlotRule => plan.slots.item[slot] ?? { sources: [], combine: 'first' };

const withRule = (plan: ImportPlan, slot: 'sets' | 'template', rule: SlotRule): ImportPlan => {
  const empty = rule.sources.length === 0 && (rule.map === undefined || Object.keys(rule.map).length === 0);
  const item = Object.fromEntries(Object.entries(plan.slots.item).filter(([name]) => name !== slot));
  return { ...plan, slots: { ...plan.slots, item: empty ? item : { ...item, [slot]: rule } } };
};

const own = (rule: SlotRule): SourceRef | undefined => rule.sources.find((source) => source.from !== 'column');

/** The folder name a new item set for everything takes. */
export const folderName = (plan: ImportPlan): string => {
  const folder = plan.sources.find((source) => source.kind === 'folder');
  if (folder?.kind !== 'folder') {
    return '';
  }
  return folder.path === '' ? folder.root : (folder.path.split('/').at(-1) ?? folder.path);
};

/** The header of the first list column that feeds the item set rule, if one does. */
export const setColumnOf = (plan: ImportPlan): string | null => {
  const source = ruleOf(plan, 'sets').sources.find((one) => one.from === 'column');
  return source?.from === 'column' ? source.column.header : null;
};

export const setChoiceOf = (plan: ImportPlan): { readonly choice: SetChoice; readonly set: string | null } => {
  const source = own(ruleOf(plan, 'sets'));
  if (source === undefined) {
    return setColumnOf(plan) === null ? { choice: 'none', set: null } : { choice: 'column', set: null };
  }
  if (source.from === 'folder') {
    return { choice: source.level === 'path' ? 'leaf' : 'top', set: null };
  }
  if (source.from === 'constant') {
    return /^\d+$/.test(source.value) ? { choice: 'old', set: source.value } : { choice: 'one', set: source.value };
  }
  return { choice: 'none', set: null };
};

export const withSets = (plan: ImportPlan, choice: SetChoice, set: string | null): ImportPlan => {
  const rule = ruleOf(plan, 'sets');
  const lists = choice === 'none' ? [] : rule.sources.filter((source) => source.from === 'column');
  const chosen: readonly SourceRef[] =
    choice === 'leaf'
      ? [{ from: 'folder', level: 'path' }]
      : choice === 'top'
        ? [{ from: 'folder', level: 1 }]
        : choice === 'one'
          ? [{ from: 'constant', value: set ?? folderName(plan) }]
          : choice === 'old' && set !== null
            ? [{ from: 'constant', value: set }]
            : [];
  return withRule(plan, 'sets', { ...rule, sources: [...lists, ...chosen] });
};

/** Where the item type comes from: nowhere, a list's column (`sheet` and index), or the top folder. */
export type TypeFrom = { readonly from: 'none' } | { readonly from: 'top' } | { readonly from: 'column'; readonly sheet: string; readonly index: number };

export const typeFromOf = (plan: ImportPlan): TypeFrom => {
  const first = ruleOf(plan, 'template').sources[0];
  if (first?.from === 'column') {
    return { from: 'column', sheet: first.sheet, index: first.column.index };
  }
  if (first?.from === 'folder') {
    return { from: 'top' };
  }
  return { from: 'none' };
};

export const withTypeFrom = (plan: ImportPlan, from: TypeFrom): ImportPlan => {
  const rule = ruleOf(plan, 'template');
  if (from.from === 'none') {
    return withRule(plan, 'template', { ...rule, sources: [] });
  }
  if (from.from === 'top') {
    return withRule(plan, 'template', { ...rule, sources: [{ from: 'folder', level: 1 }] });
  }
  const sheet = plan.sheets.find((one) => one.id === from.sheet);
  const column = sheet?.columns.find((one) => one.column.index === from.index)?.column;
  return column === undefined ? plan : withRule(plan, 'template', { ...rule, sources: [{ from: 'column', sheet: from.sheet, column }] });
};

/** The plan without the template map, so the values counted are the ones written in the files and lists. */
export const withoutTypeMap = (plan: ImportPlan): ImportPlan => {
  const rule = plan.slots.item.template;
  if (rule?.map === undefined) {
    return plan;
  }
  return { ...plan, slots: { ...plan.slots, item: { ...plan.slots.item, template: { sources: rule.sources, combine: rule.combine } } } };
};

/** What a value is mapped to: nothing chosen, no item type, or the text that names an item type. */
export type TypeAnswer = { readonly to: 'unset' } | { readonly to: 'none' } | { readonly to: 'type'; readonly label: string };

export const typeAnswerOf = (plan: ImportPlan, value: string): TypeAnswer => {
  const map = plan.slots.item.template?.map ?? {};
  const key = Object.keys(map).find((one) => fold(one) === fold(value));
  if (key === undefined) {
    return { to: 'unset' };
  }
  const label = map[key] ?? '';
  return label === '' ? { to: 'none' } : { to: 'type', label };
};

/** Maps `value` (and every spelling that folds to it) to `answer`; a new item type is added to the plan's new types. */
export const withTypeAnswer = (plan: ImportPlan, value: string, answer: TypeAnswer, isNew: boolean): ImportPlan => {
  const rule = ruleOf(plan, 'template');
  const map = Object.fromEntries(Object.entries(rule.map ?? {}).filter(([key]) => fold(key) !== fold(value)));
  if (answer.to === 'none') {
    map[value] = '';
  } else if (answer.to === 'type') {
    map[value] = answer.label;
  }
  const newTypes =
    isNew && answer.to === 'type' && !plan.newTypes.some((type) => fold(type.label) === fold(answer.label))
      ? [...plan.newTypes, { label: answer.label, from: 'mapping' as const, class: null }]
      : plan.newTypes;
  const next: SlotRule = Object.keys(map).length === 0 ? { sources: rule.sources, combine: rule.combine } : { ...rule, map };
  return withRule({ ...plan, newTypes }, 'template', next);
};
