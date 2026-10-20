/**
 * The fields a setting can name: every property of the install and the
 * named slots, in words. A term the install does not
 * have (a new field of this import) is shown by its term.
 */
import { useAsync } from '../../../app/useAsync';
import type { AsyncState } from '../../../app/useAsync';
import { isTerm } from '../../../api/import-plan';
import type { NamedSlot, SlotName, Term } from '../../../api/import-plan';
import type { Property } from '../../../api/types';
import { loadProperties } from '../lists/catalog';
import type { AdvancedMessageKey, AdvancedTranslator } from './messages';

/** The install's properties, every page of them. */
export const useProperties = (): AsyncState<readonly Property[]> => useAsync(loadProperties, []);

export const SLOT_WORDS: Readonly<Record<NamedSlot, AdvancedMessageKey>> = {
  sets: 'adv.slot.sets',
  template: 'adv.slot.template',
  class: 'adv.slot.class',
  owner: 'adv.slot.owner',
  visibility: 'adv.slot.visibility',
  sites: 'adv.slot.sites',
  mediaOrder: 'adv.slot.mediaOrder',
  ingester: 'adv.slot.ingester',
  mediaSource: 'adv.slot.mediaSource',
  itemKey: 'adv.slot.itemKey',
};

const labelOf = (term: Term, known: readonly Property[]): string => known.find((one) => one.term === term)?.label ?? term;

/** "Title (dcterms:title)", "Item set". */
export const slotWords = (slot: SlotName, known: readonly Property[], t: AdvancedTranslator['t']): string =>
  isTerm(slot) ? t('adv.slot.term', { label: labelOf(slot, known), term: slot }) : t(SLOT_WORDS[slot]);

export interface FieldOption {
  readonly value: SlotName;
  readonly label: string;
}

/** The named slots given, then every property by label. */
export const fieldOptions = (named: readonly NamedSlot[], known: readonly Property[], t: AdvancedTranslator['t']): readonly FieldOption[] => [
  ...named.map((slot) => ({ value: slot, label: t(SLOT_WORDS[slot]) })),
  ...known.flatMap((property) => (isTerm(property.term) ? [{ value: property.term, label: slotWords(property.term, known, t) }] : [])),
];

/** Finds the option for a value read back from a `<select>`. */
export const optionFor = (options: readonly FieldOption[], value: string): SlotName | null =>
  options.find((option) => option.value === value)?.value ?? null;
