/**
 * Publishing a term list or a vocabulary at a permanent address
 * (`/api/custom_vocabs/<id>/publication`, `/api/vocabularies/<id>/publication`),
 * Romeka's own routes: the parser, the commands and the calls. Only the term
 * list and vocabulary pages read it, so it stays out of the initial payload.
 */
import type { ApiClient } from './client';
import {
  ParseError,
  asBoolean,
  asNumber,
  asRecord,
  asString,
  field,
  nullableField,
  parseListOf,
} from './parse';
import type { Parser, Path, TermListId, VocabularyId } from './types';

/** What is published: a term list (SKOS) or a vocabulary (RDFS). */
export type PublicationTarget =
  | { readonly kind: 'term_list'; readonly id: TermListId }
  | { readonly kind: 'vocabulary'; readonly id: VocabularyId };

export interface PublicationVersion {
  readonly version: number;
  /** `YYYY-MM-DDTHH:MM:SS`, UTC. */
  readonly created: string;
  readonly terms: number;
}

export interface PublicationTerm {
  /** The word, link or local name the term is stored under. */
  readonly key: string;
  readonly label: string;
  readonly uri: string;
  /** A version carries the term, so it can no longer be removed. */
  readonly published: boolean;
  readonly deprecated: boolean;
  /** A version carries the deprecation, so it can no longer be taken back. */
  readonly deprecationPublished: boolean;
  readonly exactMatch: string | null;
  readonly closeMatch: string | null;
  /** The key of the term a merge moved this one's values to. */
  readonly replacedBy: string | null;
}

export interface Publication {
  readonly state: 'draft' | 'published';
  readonly kind: 'term_list' | 'vocabulary';
  /** Why this cannot be published; the other members are then empty. */
  readonly refusal: string | null;
  readonly slug: string;
  readonly slugFixed: boolean;
  readonly namespace: string;
  readonly namespaceFixed: boolean;
  /** Where this archive serves it. */
  readonly url: string;
  readonly versions: readonly PublicationVersion[];
  /** The draft differs from the newest version. */
  readonly changed: boolean;
  readonly terms: readonly PublicationTerm[];
  /** The Turtle the next publish would serve. */
  readonly preview: string;
}

/** One `POST` body. */
export type PublicationCommand =
  | { readonly action: 'publish'; readonly slug?: string; readonly namespace?: string }
  | { readonly action: 'deprecate'; readonly key: string }
  | { readonly action: 'undeprecate'; readonly key: string }
  | {
      readonly action: 'link';
      readonly key: string;
      readonly exact_match: string | null;
      readonly close_match: string | null;
    };

const oneOf = <T extends string>(choices: readonly T[], value: unknown, path: Path): T => {
  const text = asString(value, path);
  const found = choices.find((choice) => choice === text);
  if (found === undefined) {
    throw new ParseError(path, `one of ${choices.join(', ')}`);
  }
  return found;
};

const parseVersion: Parser<PublicationVersion> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    version: asNumber(field(object, 'version', path), [...path, 'version']),
    created: asString(field(object, 'created', path), [...path, 'created']),
    terms: asNumber(field(object, 'terms', path), [...path, 'terms']),
  };
};

const parseTerm: Parser<PublicationTerm> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const text = (key: string): string => asString(field(object, key, path), [...path, key]);
  const flag = (key: string): boolean => asBoolean(field(object, key, path), [...path, key]);
  return {
    key: text('key'),
    label: text('label'),
    uri: text('uri'),
    published: flag('published'),
    deprecated: flag('deprecated'),
    deprecationPublished: flag('deprecation_published'),
    exactMatch: nullableField(object, 'exact_match', path, asString),
    closeMatch: nullableField(object, 'close_match', path, asString),
    replacedBy: nullableField(object, 'replaced_by', path, asString),
  };
};

export const parsePublication: Parser<Publication> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const text = (key: string): string => asString(field(object, key, path), [...path, key]);
  const flag = (key: string): boolean => asBoolean(field(object, key, path), [...path, key]);
  return {
    state: oneOf(['draft', 'published'], field(object, 'state', path), [...path, 'state']),
    kind: oneOf(['term_list', 'vocabulary'], field(object, 'kind', path), [...path, 'kind']),
    refusal: nullableField(object, 'refusal', path, asString),
    slug: text('slug'),
    slugFixed: flag('slug_fixed'),
    namespace: text('namespace'),
    namespaceFixed: flag('namespace_fixed'),
    url: text('url'),
    versions: parseListOf(parseVersion)(field(object, 'versions', path), [...path, 'versions']),
    changed: flag('changed'),
    terms: parseListOf(parseTerm)(field(object, 'terms', path), [...path, 'terms']),
    preview: text('preview'),
  };
};

const pathOf = (target: PublicationTarget): string =>
  `/api/${target.kind === 'term_list' ? 'custom_vocabs' : 'vocabularies'}/${String(target.id)}/publication`;

export const getPublication = async (
  api: ApiClient,
  target: PublicationTarget,
): Promise<Publication> => parsePublication(await api.requestJson(pathOf(target)));

/** The keys of a term list's deprecated terms, which no new value should use. */
export const getDeprecatedKeys = async (
  api: ApiClient,
  id: TermListId,
): Promise<ReadonlySet<string>> => {
  const publication = await getPublication(api, { kind: 'term_list', id });
  return new Set(publication.terms.filter((term) => term.deprecated).map((term) => term.key));
};

export const sendPublicationCommand = async (
  api: ApiClient,
  target: PublicationTarget,
  command: PublicationCommand,
): Promise<Publication> =>
  parsePublication(await api.writeJson('POST', pathOf(target), { ...command }));
