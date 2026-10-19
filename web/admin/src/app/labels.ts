import type {
  ClassId,
  Property,
  PropertyId,
  ResourceClass,
  ResourceRef,
  User,
  UserId,
  Vocabulary,
  VocabularyId,
} from '../api/types';
import type { TermText } from '../i18n';

/**
 * A vocabulary, class or property as a list names it: its term (the prefix for
 * a vocabulary) and its stored label. The label is put in the UI's locale when
 * it is shown, so a locale switch needs no new read.
 */
export interface TermName {
  readonly term: string;
  readonly label: string;
}

export const userNames = (users: readonly User[]): ReadonlyMap<UserId, string> =>
  new Map(users.map((user) => [user.id, user.name]));

export const classLabels = (classes: readonly ResourceClass[]): ReadonlyMap<ClassId, TermName> =>
  new Map(
    classes.map((entry) => [entry.id, { term: entry.term, label: entry.label ?? entry.term }]),
  );

export const vocabularyLabels = (
  vocabularies: readonly Vocabulary[],
): ReadonlyMap<VocabularyId, TermName> =>
  new Map(
    vocabularies.map((entry) => [entry.id, { term: entry.prefix, label: entry.label || entry.prefix }]),
  );

export const propertyLabels = (properties: readonly Property[]): ReadonlyMap<PropertyId, TermName> =>
  new Map(
    properties.map((entry) => [entry.id, { term: entry.term, label: entry.label ?? entry.term }]),
  );

/** `#id` for a row that was not in the fetched page. */
const missing = (ref: ResourceRef<number>): string => `#${String(ref.id)}`;

/** The stored name, or `#id` when the row was not in the fetched page. */
export const ownerNameOf = (
  names: ReadonlyMap<UserId, string>,
  ref: ResourceRef<UserId> | null,
): string | null => (ref === null ? null : (names.get(ref.id) ?? missing(ref)));

/** The label in the locale `text` speaks, or `#id` when the row was not fetched. */
const termLabelOf =
  <IdT extends number>() =>
  (labels: ReadonlyMap<IdT, TermName>, ref: ResourceRef<IdT> | null, text: TermText): string | null => {
    if (ref === null) {
      return null;
    }
    const name = labels.get(ref.id);
    return name === undefined ? missing(ref) : text.label(name.term, name.label);
  };

export const classLabelOf = termLabelOf<ClassId>();
export const vocabularyLabelOf = termLabelOf<VocabularyId>();
export const propertyLabelOf = termLabelOf<PropertyId>();
