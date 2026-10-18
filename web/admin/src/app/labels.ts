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

export const userNames = (users: readonly User[]): ReadonlyMap<UserId, string> =>
  new Map(users.map((user) => [user.id, user.name]));

export const classLabels = (classes: readonly ResourceClass[]): ReadonlyMap<ClassId, string> =>
  new Map(classes.map((resourceClass) => [resourceClass.id, resourceClass.label ?? resourceClass.term]));

export const vocabularyLabels = (
  vocabularies: readonly Vocabulary[],
): ReadonlyMap<VocabularyId, string> =>
  new Map(vocabularies.map((vocabulary) => [vocabulary.id, vocabulary.label || vocabulary.prefix]));

export const propertyLabels = (properties: readonly Property[]): ReadonlyMap<PropertyId, string> =>
  new Map(properties.map((property) => [property.id, property.label ?? property.term]));

/** The upstream label, or `#id` when the row was not in the fetched page. */
const labelOf = <IdT extends number>(
  labels: ReadonlyMap<IdT, string>,
  ref: ResourceRef<IdT> | null,
): string | null => (ref === null ? null : (labels.get(ref.id) ?? `#${String(ref.id)}`));

export const ownerNameOf = labelOf<UserId>;
export const classLabelOf = labelOf<ClassId>;
export const vocabularyLabelOf = labelOf<VocabularyId>;
export const propertyLabelOf = labelOf<PropertyId>;
