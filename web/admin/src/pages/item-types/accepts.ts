/**
 * What a field accepts, in plain words. A row's data types are upstream data
 * type names; the editor offers every type `/api/data_types` lists, so a
 * module's types appear, and keeps a stored type the server no longer lists.
 */
import { isTermListName } from '../../api/term-lists';
import type { DataType } from '../../api/types';

/** The plain-words message for a type the core registers. */
export const CORE_ACCEPTS = {
  literal: 'accepts.literal',
  uri: 'accepts.uri',
  resource: 'accepts.resource',
  'resource:item': 'accepts.item',
  'resource:itemset': 'accepts.itemSet',
  'resource:media': 'accepts.media',
} as const;

export type CoreAccepts = keyof typeof CORE_ACCEPTS;

export const isCoreAccepts = (type: string): type is CoreAccepts => type in CORE_ACCEPTS;

/** Every type a row may offer: the registered ones in their order, then the row's others. */
export const acceptChoices = (
  dataTypes: readonly DataType[],
  stored: readonly string[],
): readonly string[] => [
  ...dataTypes.map((dataType) => dataType.id),
  ...stored.filter((id) => !dataTypes.some((dataType) => dataType.id === id)),
];

/**
 * The choices split the way the Custom Vocab module's optgroup splits them
 * (`CustomVocab::getOptgroupLabel`): the other types, then the term lists. An
 * empty group is left out.
 */
export const acceptGroups = (
  choices: readonly string[],
): readonly { readonly group: 'kinds' | 'term-lists'; readonly types: readonly string[] }[] =>
  [
    { group: 'kinds' as const, types: choices.filter((id) => !isTermListName(id)) },
    { group: 'term-lists' as const, types: choices.filter(isTermListName) },
  ].filter((entry) => entry.types.length > 0);

export const isInstalled = (dataTypes: readonly DataType[], id: string): boolean =>
  dataTypes.some((dataType) => dataType.id === id);
