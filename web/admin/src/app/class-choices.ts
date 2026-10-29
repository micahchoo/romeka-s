/**
 * How the admin names a class, and the class filter's choices.
 *
 * A class's name is its label in the reader's language. When another class
 * of the same list has the same label, the name also carries the class's
 * standard: "Image (Dublin Core Type)". Every place that shows a class name
 * builds it with `classNames` and words it with `useClassNameText`.
 *
 * The filter's choices, wherever resources are filtered by class (the Items
 * browse, the item picker and a block's query), are the classes at least one
 * resource of that type the caller may see has, with the exact count
 * (`GET /api/romeka/classes/counts`), so a filter never offers a class that
 * lists nothing. A class the filter already holds stays a choice even when no
 * resource has it, at 0, so a stored query or a link never loses its class
 * silently.
 */
import { api } from '../api/client';
import { placesApi } from '../api/places';
import type { ClassCount, ClassCountType } from '../api/places';
import type { ClassId, ResourceClass, Vocabulary, VocabularyId } from '../api/types';
import { getVocabulary } from '../api/vocabularies';
import { useTranslator } from '../i18n';
import type { KeyOf } from '../i18n';
import { classFilter } from '../i18n/namespaces/class-filter';
import { useTermLabels } from '../i18n/termText';
import type { TermLabels } from '../i18n/termText';
import { useAsync } from './useAsync';

/** What a class name is built from. */
export interface NamedClass {
  readonly id: ClassId;
  /** `prefix:local_name`. */
  readonly term: string;
  /** The stored label; two classes of different standards may share it. */
  readonly label: string;
  readonly vocabularyLabel: string;
}

/** A class's name in the reader's language. */
export interface ClassName {
  readonly label: string;
  /** The standard's label, present only when another class of the list has the same label. */
  readonly standard: string | null;
}

/** One class as the filter offers it. */
export interface ClassChoice extends ClassName {
  readonly id: ClassId;
  readonly count: number;
}

const prefixOf = (term: string): string => term.slice(0, Math.max(term.indexOf(':'), 0));

/** Each class's name, told apart from the other classes of `classes` that share its label. */
export const classNames = (classes: readonly NamedClass[], text: TermLabels): ReadonlyMap<ClassId, ClassName> => {
  const named = classes.map((entry) => ({ ...entry, shown: text.label(entry.term, entry.label) }));
  const uses = new Map<string, number>();
  for (const { shown } of named) {
    uses.set(shown, (uses.get(shown) ?? 0) + 1);
  }
  return new Map(
    named.map(({ id, term, vocabularyLabel, shown }) => [
      id,
      { label: shown, standard: (uses.get(shown) ?? 0) > 1 ? text.label(prefixOf(term), vocabularyLabel) : null },
    ]),
  );
};

/** Classes as `classNames` reads them, each with its standard's stored label. */
export const namedClasses = (
  classes: readonly ResourceClass[],
  vocabularies: readonly Vocabulary[],
): readonly NamedClass[] => {
  const standards = new Map<VocabularyId, string>(vocabularies.map((entry) => [entry.id, entry.label || entry.prefix]));
  return classes.map((entry) => ({
    id: entry.id,
    term: entry.term,
    label: entry.label ?? entry.localName,
    vocabularyLabel: standards.get(entry.vocabulary.id) ?? prefixOf(entry.term),
  }));
};

/** The choices, in the order of `counts` (the server's: by label, then standard). */
export const classChoices = (counts: readonly ClassCount[], text: TermLabels): readonly ClassChoice[] => {
  const names = classNames(counts, text);
  return counts.map(({ id, count }) => ({ id, count, ...(names.get(id) ?? { label: `#${String(id)}`, standard: null }) }));
};

/** A held class no resource has, read by id so the filter can name it at 0. */
const unusedClass = async (id: ClassId): Promise<ClassCount> => {
  const resourceClass = await api.getResourceClass(id);
  const vocabulary = await getVocabulary(api, resourceClass.vocabulary.id);
  return {
    id,
    term: resourceClass.term,
    label: resourceClass.label ?? resourceClass.localName,
    vocabularyLabel: vocabulary.label,
    count: 0,
  };
};

/**
 * The filter's choices for `resource`, with `held` among them. `loaded` is the
 * counts when the caller reads them: the hook then asks for none, and `null`
 * means the caller's read has not answered yet. A held class that cannot be
 * read (deleted, or hidden from the caller) is named by its id.
 */
export const useClassChoices = (
  held: ClassId | null,
  resource: ClassCountType = 'items',
  loaded?: readonly ClassCount[] | null,
): readonly ClassChoice[] => {
  const text = useTermLabels();
  const counts = useAsync(
    async () => (loaded === undefined ? await placesApi.classCounts(resource) : loaded),
    [loaded, resource],
  );
  const used = counts.status === 'ready' ? (counts.data ?? []) : [];
  const ready = counts.status === 'ready' && counts.data !== null;
  const missing = ready && held !== null && !used.some((count) => count.id === held) ? held : null;
  const extra = useAsync(async () => (missing === null ? null : await unusedClass(missing)), [missing]);
  const choices = classChoices(extra.status === 'ready' && extra.data !== null ? [...used, extra.data] : used, text);
  return missing !== null && extra.status === 'error'
    ? [...choices, { id: missing, label: `#${String(missing)}`, standard: null, count: 0 }]
    : choices;
};

/** A class name's words: "Image", or "Image (Dublin Core Type)" when the label is shared. */
export const useClassNameText = (): ((name: ClassName) => string) => {
  const { t } = useTranslator(classFilter);
  return (name) =>
    name.standard === null ? name.label : t('classFilter.standard', { label: name.label, standard: name.standard });
};

type OptionKey = KeyOf<typeof classFilter>;

/** A choice's words by the type it counts: one resource, and several. */
const OPTION_WORDS: Readonly<Record<ClassCountType, { readonly one: OptionKey; readonly other: OptionKey }>> = {
  items: { one: 'classFilter.optionOne', other: 'classFilter.option' },
  item_sets: { one: 'classFilter.optionItemSetOne', other: 'classFilter.optionItemSets' },
  media: { one: 'classFilter.optionMediaOne', other: 'classFilter.optionMedia' },
};

/** A choice's option text: "Image (Dublin Core Type) · 3 items", or "· 3 media" for media. */
export const useClassOptionText = (resource: ClassCountType = 'items'): ((choice: ClassChoice) => string) => {
  const { t, number } = useTranslator(classFilter);
  const nameText = useClassNameText();
  const words = OPTION_WORDS[resource];
  return (choice) => {
    const label = nameText(choice);
    return choice.count === 1 ? t(words.one, { label }) : t(words.other, { label, count: number(choice.count) });
  };
};
