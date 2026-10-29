/**
 * The class filter's choices wherever items are filtered by class: the Items
 * browse, the item picker and a block's item query. Each choice is a class
 * that at least one item the caller may see has, with its exact count
 * (`GET /api/romeka/classes/counts`), so a filter never offers a class that
 * lists nothing. A name two such classes share carries each one's standard.
 *
 * A class the filter already holds stays a choice even when no item has it,
 * at 0 items, so a stored query or a link never loses its class silently.
 */
import { api } from '../api/client';
import { placesApi } from '../api/places';
import type { ClassCount } from '../api/places';
import type { ClassId } from '../api/types';
import { getVocabulary } from '../api/vocabularies';
import { useTranslator } from '../i18n';
import { classFilter } from '../i18n/namespaces/class-filter';
import { useTermLabels } from '../i18n/termText';
import type { TermLabels } from '../i18n/termText';
import { useAsync } from './useAsync';

/** One class as the filter names it. */
export interface ClassChoice {
  readonly id: ClassId;
  /** The class's label in the reader's language. */
  readonly label: string;
  /** The standard's label, present only when another choice has the same label. */
  readonly standard: string | null;
  readonly items: number;
}

const prefixOf = (term: string): string => term.slice(0, Math.max(term.indexOf(':'), 0));

/** The choices, in the order of `counts` (the server's: by label, then standard). */
export const classChoices = (counts: readonly ClassCount[], text: TermLabels): readonly ClassChoice[] => {
  const named = counts.map((count) => ({ ...count, label: text.label(count.term, count.label) }));
  const uses = new Map<string, number>();
  for (const { label } of named) {
    uses.set(label, (uses.get(label) ?? 0) + 1);
  }
  return named.map(({ id, term, label, vocabularyLabel, items }) => ({
    id,
    label,
    standard: (uses.get(label) ?? 0) > 1 ? text.label(prefixOf(term), vocabularyLabel) : null,
    items,
  }));
};

/** A held class no item has, read by id so the filter can name it at 0 items. */
const unusedClass = async (id: ClassId): Promise<ClassCount> => {
  const resourceClass = await api.getResourceClass(id);
  const vocabulary = await getVocabulary(api, resourceClass.vocabulary.id);
  return {
    id,
    term: resourceClass.term,
    label: resourceClass.label ?? resourceClass.localName,
    vocabularyLabel: vocabulary.label,
    items: 0,
  };
};

/**
 * The filter's choices, with `held` among them. `loaded` is the counts when
 * the caller already read them; the hook then asks for none. A held class
 * that cannot be read (deleted, or hidden from the caller) is named by its id.
 */
export const useClassChoices = (
  held: ClassId | null,
  loaded?: readonly ClassCount[],
): readonly ClassChoice[] => {
  const text = useTermLabels();
  const counts = useAsync(async () => loaded ?? (await placesApi.classCounts()), [loaded]);
  const used = counts.status === 'ready' ? counts.data : [];
  const missing = counts.status === 'ready' && held !== null && !used.some((count) => count.id === held) ? held : null;
  const extra = useAsync(async () => (missing === null ? null : await unusedClass(missing)), [missing]);
  const choices = classChoices(extra.status === 'ready' && extra.data !== null ? [...used, extra.data] : used, text);
  return missing !== null && extra.status === 'error'
    ? [...choices, { id: missing, label: `#${String(missing)}`, standard: null, items: 0 }]
    : choices;
};

/** A choice's option text: "Image (Dublin Core Type) · 3 items". */
export const useClassOptionText = (): ((choice: ClassChoice) => string) => {
  const { t, number } = useTranslator(classFilter);
  return (choice) => {
    const label = choice.standard === null ? choice.label : t('classFilter.standard', { label: choice.label, standard: choice.standard });
    return choice.items === 1
      ? t('classFilter.optionOne', { label })
      : t('classFilter.option', { label, count: number(choice.items) });
  };
};
