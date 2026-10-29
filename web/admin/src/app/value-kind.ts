import type { DataType } from '../api/types';

const CORE_KIND_WORDS = {
  literal: 'field.kindText',
  uri: 'field.kindUri',
  resource: 'field.kindResource',
} as const;

type KindWordId = (typeof CORE_KIND_WORDS)[keyof typeof CORE_KIND_WORDS];

const isCoreType = (type: string): type is keyof typeof CORE_KIND_WORDS => type in CORE_KIND_WORDS;

/**
 * The word for a value's data type, the same on the read list and in the form: the core three
 * by the form's words, a module's type by its registered label, else its name.
 */
export const valueKindWord = (
  type: string,
  dataTypes: readonly DataType[],
  t: (id: KindWordId) => string,
): string => (isCoreType(type) ? t(CORE_KIND_WORDS[type]) : (dataTypes.find((entry) => entry.id === type)?.label ?? type));
