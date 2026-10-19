/**
 * "Create a new field": a property in this archive's own vocabulary, made the
 * first time someone needs a field no standard names.
 *
 * The vocabulary is an ordinary upstream vocabulary, prefix `local`, whose
 * namespace sits under the installation's own address
 * (`<origin>/vocab/local/`), so its terms resolve to this archive and another
 * Omeka S reads them like any other vocabulary.
 */
import { api } from '../api/client';
import type { Property, Vocabulary } from '../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from './all-pages';

export const LOCAL_PREFIX = 'local';

export const localNamespace = (origin: string): string => `${origin}/vocab/local/`;

/** A term of this archive's own vocabulary, such as `local:shelfMark`. */
export const isLocalTerm = (term: string): boolean => term.startsWith(`${LOCAL_PREFIX}:`);

/**
 * The local name for a field named in plain words: ASCII camel case, as the
 * standard vocabularies write theirs ("Number of pages" is `numberOfPages`).
 */
export const localNameFor = (name: string): string => {
  const parts = name
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .split(/[^A-Za-z0-9]+/)
    .filter((part) => part !== '');
  const camel = parts
    .map((part, index) =>
      index === 0 ? part.toLowerCase() : part.charAt(0).toUpperCase() + part.slice(1).toLowerCase(),
    )
    .join('');
  return camel === '' || /^[0-9]/.test(camel) ? `field${camel}` : camel;
};

const localVocabulary = async (): Promise<Vocabulary> => {
  const vocabularies = await allPages((page) =>
    api.listVocabularies({ page, perPage: ALL_PAGES_PER_PAGE }),
  );
  const found = vocabularies.find((vocabulary) => vocabulary.prefix === LOCAL_PREFIX);
  return (
    found ??
    api.createVocabulary({
      namespaceUri: localNamespace(window.location.origin),
      prefix: LOCAL_PREFIX,
      label: 'Local fields',
      comment: 'Fields this archive made for itself.',
      owner: null,
    })
  );
};

/**
 * The property for a new field, created with its vocabulary on first use. A
 * name whose term already exists returns that property instead of a copy.
 */
export const createLocalField = async (
  name: string,
  properties: readonly Property[],
): Promise<Property> => {
  const localName = localNameFor(name);
  const existing = properties.find((property) => property.term === `${LOCAL_PREFIX}:${localName}`);
  if (existing !== undefined) {
    return existing;
  }
  const vocabulary = await localVocabulary();
  return api.createProperty({
    localName,
    label: name.trim(),
    comment: null,
    owner: null,
    vocabulary: vocabulary.id,
  });
};
