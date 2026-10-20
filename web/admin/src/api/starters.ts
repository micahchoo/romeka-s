/**
 * The item type starters (`GET /api/romeka/starters`), Romeka's own route. The
 * server owns the one table: the import run creates templates from it and the
 * Item types and Import pages list the same rows. The shape is the contract in
 * `docs/archive-import.md`.
 */
import type { ApiClient } from './client';
import { ParseError, asBoolean, asOneOf, asRecord, asString, asStringList, field, parseListOf } from './parse';
import type { Parser } from './types';

/** The ids a plan stores for a new type. */
export type StarterId = 'letter' | 'photograph' | 'postcard' | 'diary' | 'person' | 'other';
export const STARTER_IDS: readonly StarterId[] = ['letter', 'photograph', 'postcard', 'diary', 'person', 'other'];

export interface StarterField {
  readonly term: string;
  readonly required: boolean;
  /** The label the template shows in place of the property's own, in English. */
  readonly label: string;
  readonly dataTypes: readonly string[];
}

export interface Starter {
  readonly id: StarterId;
  /** The key a locale translates the name by; `name` is its English text. */
  readonly nameKey: string;
  readonly name: string;
  /** What a user may type for this kind, in any shipped locale. */
  readonly words: readonly string[];
  readonly classTerm: string;
  /** The first field is the required name, and the type's title property. */
  readonly fields: readonly StarterField[];
}

const parseField: Parser<StarterField> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    term: asString(field(object, 'term', path), [...path, 'term']),
    required: asBoolean(field(object, 'required', path), [...path, 'required']),
    label: asString(field(object, 'label', path), [...path, 'label']),
    dataTypes: asStringList(field(object, 'dataTypes', path), [...path, 'dataTypes']),
  };
};

const parseStarter: Parser<Starter> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: asOneOf(STARTER_IDS, field(object, 'id', path), [...path, 'id']),
    nameKey: asString(field(object, 'nameKey', path), [...path, 'nameKey']),
    name: asString(field(object, 'name', path), [...path, 'name']),
    words: asStringList(field(object, 'words', path), [...path, 'words']),
    classTerm: asString(field(object, 'classTerm', path), [...path, 'classTerm']),
    fields: parseListOf(parseField)(field(object, 'fields', path), [...path, 'fields']),
  };
};

/** The server's starters, and the `other` one an unknown kind starts as. */
export interface StarterTable {
  readonly starters: readonly Starter[];
  readonly other: Starter;
}

export const parseStarters: Parser<StarterTable> = (raw, path = []) => {
  const starters = parseListOf(parseStarter)(raw, path);
  const other = starters.find((starter) => starter.id === 'other');
  if (other === undefined) {
    throw new ParseError(path, 'a starter with the id other');
  }
  return { starters, other };
};

export const getStarters = async (api: ApiClient): Promise<StarterTable> =>
  parseStarters(await api.requestJson('/api/romeka/starters'));
