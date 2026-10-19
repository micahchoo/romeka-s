/**
 * Term lists (`/api/custom_vocabs`, the Custom Vocab module built into core):
 * the parser, the encoder and the calls. Only lazy pages read term lists, so
 * this module stays out of the initial payload that `client.ts` is part of.
 */
import type { ApiClient, ResourceQuery } from './client';
import {
  ParseError,
  asIdNumber,
  asRecord,
  asRef,
  asRefUser,
  asString,
  brandId,
  field,
  isRecord,
  nullableField,
  parseListOf,
  positiveInt,
} from './parse';
import type {
  ApiPage,
  Path,
  Parser,
  TermList,
  TermListId,
  TermListName,
  TermListSource,
  TermListUri,
  TermListWrite,
} from './types';
import { ref } from './write';
import type { WireBody } from './write';

/** Whether a data type name names a term list (`^customvocab:(\d+)$`). */
export const isTermListName = (name: string): name is TermListName =>
  /^customvocab:\d+$/.test(name);

const termListName = (id: number): TermListName => {
  const name = `customvocab:${String(id)}`;
  if (!isTermListName(name)) {
    throw new ParseError([], 'a term list id');
  }
  return name;
};

/** Parses a term list id out of a URL fragment such as `#/term-lists/3`. */
export function termListIdFromParam(text: string): TermListId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'TermListId'>(value);
}

const parseTermListUris = (value: unknown, path: Path): readonly TermListUri[] =>
  Object.entries(asRecord(value, path)).map(([uri, label]) => ({
    uri,
    label: asString(label, [...path, uri]),
  }));

/** `CustomVocabRepresentation::type`: an item set, then URIs, then terms. */
const parseTermListSource = (object: Record<string, unknown>, path: Path): TermListSource => {
  const itemSet = nullableField(object, 'o:item_set', path, (value, at) =>
    asRef(value, at, brandId<'ItemSetId'>),
  );
  if (itemSet !== null) {
    return { kind: 'items', itemSet };
  }
  const rawUris = field(object, 'o:uris', path);
  if (isRecord(rawUris)) {
    const uris = parseTermListUris(rawUris, [...path, 'o:uris']);
    if (uris.length > 0) {
      return { kind: 'uris', uris };
    }
  }
  const terms = nullableField(object, 'o:terms', path, parseListOf(asString)) ?? [];
  return terms.length > 0 ? { kind: 'terms', terms } : { kind: 'empty' };
};

export const parseTermList: Parser<TermList> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const id = asIdNumber(field(object, 'o:id', path), [...path, 'o:id']);
  return {
    id: brandId<'TermListId'>(id),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    dataType: termListName(id),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
    lang: nullableField(object, 'o:lang', path, asString),
    source: parseTermListSource(object, path),
    owner: nullableField(object, 'o:owner', path, asRefUser),
  };
};

/**
 * `CustomVocabAdapter::hydrate`. The source the form chose is sent and the
 * other two are cleared, as the module's own form does, so a list never keeps
 * a hidden source that outranks the one shown.
 */
export const encodeTermList = (write: TermListWrite): WireBody => ({
  'o:label': write.label,
  'o:lang': write.lang ?? '',
  'o:item_set': write.source.kind === 'items' ? ref(write.source.itemSet) : null,
  'o:terms': write.source.kind === 'terms' ? write.source.terms : null,
  'o:uris':
    write.source.kind === 'uris'
      ? Object.fromEntries(write.source.uris.map((entry) => [entry.uri, entry.label]))
      : null,
});

export const listTermLists = (
  api: ApiClient,
  query: ResourceQuery = {},
): Promise<ApiPage<TermList>> => api.searchPage('custom_vocabs', query, parseTermList);

export const getTermList = async (api: ApiClient, id: TermListId): Promise<TermList> =>
  parseTermList(await api.requestJson(`/api/custom_vocabs/${String(id)}`));

export const createTermList = async (api: ApiClient, write: TermListWrite): Promise<TermList> =>
  parseTermList(await api.writeJson('POST', '/api/custom_vocabs', encodeTermList(write)));

export const updateTermList = async (
  api: ApiClient,
  id: TermListId,
  write: TermListWrite,
): Promise<TermList> =>
  parseTermList(
    await api.writeJson('PATCH', `/api/custom_vocabs/${String(id)}`, encodeTermList(write)),
  );

export const deleteTermList = (api: ApiClient, id: TermListId): Promise<void> =>
  api.deleteJson(`/api/custom_vocabs/${String(id)}`);
