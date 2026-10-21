/**
 * Vocabulary documents (`/api/vocabularies`): the parser, the diff parser and
 * the calls. Only a lazy route reads a vocabulary, so this module stays out of
 * the initial payload `client.ts` is part of, the way `term-lists.ts` does.
 *
 * Classes, properties and templates keep their calls in `client.ts`: every
 * form's option lists read them, and they share `parseVocabularyMember`, which
 * stays eager with those parsers.
 */
import type { ApiClient, ResourceQuery } from './client';
import {
  asIdNumber,
  asRecord,
  asRefUser,
  asString,
  brandId,
  field,
  nullableField,
  parseListOf,
  parseTypes,
} from './parse';
import type {
  ApiPage,
  Path,
  Parser,
  Vocabulary,
  VocabularyDiff,
  VocabularyId,
  VocabularyMemberDiff,
  VocabularySource,
  VocabularyTerm,
  VocabularyTextChange,
  VocabularyWrite,
} from './types';
import {
  encodeVocabulary,
  encodeVocabularyDiff,
  encodeVocabularyImport,
  encodeVocabularySource,
} from './write';

export const parseVocabulary: Parser<Vocabulary> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'VocabularyId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    namespaceUri: asString(field(object, 'o:namespace_uri', path), [...path, 'o:namespace_uri']),
    prefix: asString(field(object, 'o:prefix', path), [...path, 'o:prefix']),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
    comment: nullableField(object, 'o:comment', path, asString),
    owner: nullableField(object, 'o:owner', path, asRefUser),
  };
};

const parseVocabularyTerm: Parser<VocabularyTerm> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    localName: asString(field(object, 'o:local_name', path), [...path, 'o:local_name']),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
    comment: nullableField(object, 'o:comment', path, asString),
  };
};

const parseTextChange =
  <T>(parse: (value: unknown, path: Path) => T): Parser<VocabularyTextChange<T>> =>
  (raw, path = []) => {
    const object = asRecord(raw, path);
    return {
      localName: asString(field(object, 'o:local_name', path), [...path, 'o:local_name']),
      from: parse(field(object, 'from', path), [...path, 'from']),
      to: parse(field(object, 'to', path), [...path, 'to']),
    };
  };

const asNullableString = (value: unknown, path: Path): string | null =>
  value === null ? null : asString(value, path);

const parseMemberDiff: Parser<VocabularyMemberDiff> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const list = <T>(key: string, parse: Parser<T>): readonly T[] =>
    parseListOf(parse)(field(object, key, path), [...path, key]);
  return {
    added: list('new', parseVocabularyTerm),
    label: list('label', parseTextChange(asString)),
    comment: list('comment', parseTextChange(asNullableString)),
    removed: list('removed', (entry, at = []) => {
      const kept = asRecord(entry, at);
      return {
        localName: asString(field(kept, 'o:local_name', at), [...at, 'o:local_name']),
        label: asString(field(kept, 'o:label', at), [...at, 'o:label']),
      };
    }),
  };
};

/** The answer of `POST /api/vocabularies/{id}/diff`. */
export const parseVocabularyDiff: Parser<VocabularyDiff> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    classes: parseMemberDiff(field(object, 'classes', path), [...path, 'classes']),
    properties: parseMemberDiff(field(object, 'properties', path), [...path, 'properties']),
  };
};

export const getVocabulary = async (api: ApiClient, id: VocabularyId): Promise<Vocabulary> =>
  parseVocabulary(await api.requestJson(`/api/vocabularies/${String(id)}`));

export const listVocabularies = (
  api: ApiClient,
  query: ResourceQuery = {},
): Promise<ApiPage<Vocabulary>> => api.searchPage('vocabularies', query, parseVocabulary);

export const createVocabulary = async (
  api: ApiClient,
  write: VocabularyWrite,
): Promise<Vocabulary> =>
  parseVocabulary(
    await api.writeJson('POST', '/api/vocabularies', encodeVocabulary(write, 'create')),
  );

/**
 * A partial update. A full `PUT` must carry the `o:class` and `o:property`
 * lists, and one that omits either removes every member of that kind, so the
 * vocabulary form sends the scalar members only.
 */
export const updateVocabulary = async (
  api: ApiClient,
  id: VocabularyId,
  write: VocabularyWrite,
): Promise<Vocabulary> =>
  parseVocabulary(
    await api.writeJson(
      'PATCH',
      `/api/vocabularies/${String(id)}`,
      encodeVocabulary(write, 'update'),
    ),
  );

export const deleteVocabulary = async (api: ApiClient, id: VocabularyId): Promise<void> =>
  api.deleteJson(`/api/vocabularies/${String(id)}`);

/**
 * Create a vocabulary from RDF (a Romeka route): the server reads the file or
 * fetches the URL and stores every class and property in the namespace.
 */
export const importVocabulary = async (
  api: ApiClient,
  write: VocabularyWrite,
  source: VocabularySource,
): Promise<Vocabulary> =>
  parseVocabulary(
    await api.writeJson(
      'POST',
      '/api/vocabularies/import',
      encodeVocabularyImport(write, source),
    ),
  );

/** What an update from this source would change (a Romeka route). Nothing is written. */
export const diffVocabulary = async (
  api: ApiClient,
  id: VocabularyId,
  source: VocabularySource,
): Promise<VocabularyDiff> =>
  parseVocabularyDiff(
    await api.writeJson('POST', `/api/vocabularies/${String(id)}/diff`, {
      source: encodeVocabularySource(source),
    }),
  );

/** Apply a reviewed diff (a Romeka route). A change that no longer applies is refused. */
export const applyVocabularyDiff = async (
  api: ApiClient,
  id: VocabularyId,
  diff: VocabularyDiff,
): Promise<Vocabulary> =>
  parseVocabulary(
    await api.writeJson(
      'POST',
      `/api/vocabularies/${String(id)}/diff/apply`,
      encodeVocabularyDiff(diff),
    ),
  );
