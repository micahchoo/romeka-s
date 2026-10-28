/**
 * The fields the Items sort list offers. Identifier comes first, because a shelf of things is
 * ordered by its numbers; then the chosen item type's own fields. A field sort orders by the
 * field's values in shelf order (`AF_2` before `AF_10`), which the server computes.
 */
import { api } from '../../api/client';
import { fieldQuery } from '../../api/places';
import type { ItemSetId, PropertyId, TemplateId } from '../../api/types';
import { useAsync } from '../../app/useAsync';
import type { TermLabels } from '../../i18n/termText';
import { isFieldSort, isTerm } from './browse';
import type { FieldSort, Sort } from './browse';
import { fieldName } from './browse-options';
import type { BrowseOptions } from './browse-options';

export const IDENTIFIER: FieldSort = 'dcterms:identifier';

/** A field the sort list offers: the term the URL names and the name the reader sees. */
export interface SortFieldChoice {
  readonly term: FieldSort;
  readonly name: string;
}

export interface SortFieldContext {
  readonly template: TemplateId | null;
  readonly chosen: Sort;
  /** Whether the set the list is filtered to has items with an identifier. */
  readonly setHasIdentifier: boolean;
}

/** The property a term names, among the properties the screen loaded. */
export const propertyOf = (options: BrowseOptions, term: string): PropertyId | null =>
  [...options.properties].find(([, name]) => name.term === term)?.[0] ?? null;

/**
 * Identifier when any item type has it or the set's items have one, then the chosen item type's
 * other fields by name, and the chosen field kept so the control always shows the order in use.
 */
export const sortFields = (
  options: BrowseOptions,
  { template, chosen, setHasIdentifier }: SortFieldContext,
  text: TermLabels,
): readonly SortFieldChoice[] => {
  const identifier = propertyOf(options, IDENTIFIER);
  const anyTypeHasIdentifier =
    identifier !== null &&
    [...options.templateFields.values()].some((fields) => fields.some((field) => field.property === identifier));
  const choice = (property: PropertyId): SortFieldChoice | null => {
    const term = options.properties.get(property)?.term;
    return term === undefined || !isTerm(term) ? null : { term, name: fieldName(options, template, property, text) };
  };
  const first = identifier !== null && (anyTypeHasIdentifier || setHasIdentifier) ? [choice(identifier)] : [];
  const own = (template === null ? [] : (options.templateFields.get(template) ?? []))
    .filter((field) => field.property !== identifier)
    .map((field) => choice(field.property))
    .filter((entry) => entry !== null)
    .sort((a, b) => a.name.localeCompare(b.name));
  const offered = [...first.filter((entry) => entry !== null), ...own];
  if (!isFieldSort(chosen) || offered.some((entry) => entry.term === chosen)) {
    return offered;
  }
  const property = propertyOf(options, chosen);
  const kept = property === null ? null : choice(property);
  return [...offered, kept ?? { term: chosen, name: chosen }];
};

/**
 * Whether any of the set's items has an identifier the reader may see: one request, a page of one
 * row, read again when the set or the revision moves. `false` while it loads or when it fails, so
 * the list never offers what it has not seen.
 */
export const useSetHasIdentifier = (
  itemSetId: ItemSetId | null,
  identifier: PropertyId | null,
  revision: number,
): boolean => {
  const state = useAsync(
    () =>
      itemSetId === null || identifier === null
        ? Promise.resolve(false)
        : api
            .listItems({ itemSetId, perPage: 1, page: 1, ...fieldQuery(identifier, 'filled') })
            .then((page) => page.total > 0),
    [itemSetId, identifier, revision],
  );
  return state.status === 'ready' && state.data;
};
