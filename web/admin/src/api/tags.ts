/**
 * Tags and the step from tag to term (`/api/tags`, `/api/custom_vocabs/<id>/values`
 * and their writes, Romeka routes). A tag is a plain-text `dcterms:subject`
 * value. Only lazy chunks read these, so this module stays out of the initial
 * payload.
 */
import type { ApiClient } from './client';
import { asNumber, asRecord, asString, brandId, field, isRecord, parseListOf } from './parse';
import type { Path, Parser, TermListId } from './types';

/** The property a tag is stored on, and its data type. */
export const TAG_TERM = 'dcterms:subject';
export const TAG_TYPE = 'literal';

/** The spellings an operation works over: every tag, or one list's words. */
export type ValueScope =
  | { readonly kind: 'tags' }
  | { readonly kind: 'list'; readonly id: TermListId };

/** One spelling and how many resources carry it. */
export interface ValueCount {
  readonly value: string;
  /** Resources the caller can see that carry it. */
  readonly resources: number;
  /** Of those, the ones the caller may edit. */
  readonly editable: number;
}

export interface Census {
  /** Most used first. */
  readonly values: readonly ValueCount[];
  /** Groups of spellings someone said are different. */
  readonly distinct: readonly (readonly string[])[];
}

/** What a write did or would do, in resources. */
export interface Rewrite {
  readonly changed: number;
  /** Resources carrying a named spelling that the caller may not edit; they keep it. */
  readonly skipped: number;
  /**
   * On a published list, the merged words that stay in it as deprecated terms,
   * because a published version carries them.
   */
  readonly deprecated: readonly string[];
}

export interface Named {
  readonly id: number;
  readonly label: string;
}

export interface Promotion extends Rewrite {
  /** `null` on a preview that would create the list. */
  readonly list: { readonly id: TermListId; readonly label: string } | null;
  readonly created: boolean;
  /** Item types whose subject field offers the list afterwards. */
  readonly templates: readonly Named[];
  /** Item types of changed items that the caller may not edit. */
  readonly templatesSkipped: readonly Named[];
}

const count = (object: Record<string, unknown>, name: string, path: Path): number =>
  asNumber(field(object, name, path), [...path, name]);

const parseValueCount: Parser<ValueCount> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    value: asString(field(object, 'value', path), [...path, 'value']),
    resources: count(object, 'resources', path),
    editable: count(object, 'editable', path),
  };
};

export const parseCensus: Parser<Census> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    values: parseListOf(parseValueCount)(field(object, 'values', path), [...path, 'values']),
    distinct: parseListOf(parseListOf(asString))(field(object, 'distinct', path), [
      ...path,
      'distinct',
    ]),
  };
};

export const parseRewrite: Parser<Rewrite> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const deprecated = object['deprecated'];
  return {
    changed: count(object, 'changed', path),
    skipped: count(object, 'skipped', path),
    deprecated:
      deprecated === undefined ? [] : parseListOf(asString)(deprecated, [...path, 'deprecated']),
  };
};

const parseNamed: Parser<Named> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: asNumber(field(object, 'o:id', path), [...path, 'o:id']),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
  };
};

export const parsePromotion: Parser<Promotion> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const list = field(object, 'list', path);
  const named = isRecord(list) ? parseNamed(list, [...path, 'list']) : null;
  return {
    ...parseRewrite(raw, path),
    list: named === null ? null : { id: brandId<'TermListId'>(named.id), label: named.label },
    created: field(object, 'created', path) === true,
    templates: parseListOf(parseNamed)(field(object, 'templates', path), [...path, 'templates']),
    templatesSkipped: parseListOf(parseNamed)(field(object, 'templates_skipped', path), [
      ...path,
      'templates_skipped',
    ]),
  };
};

const base = (scope: ValueScope): string =>
  scope.kind === 'tags' ? '/api/tags' : `/api/custom_vocabs/${String(scope.id)}`;

export const getCensus = async (api: ApiClient, scope: ValueScope): Promise<Census> =>
  parseCensus(
    await api.requestJson(scope.kind === 'tags' ? base(scope) : `${base(scope)}/values`),
  );

/** Every value spelled one of `from` becomes `to`; a list's own words follow. */
export const mergeValues = async (
  api: ApiClient,
  scope: ValueScope,
  write: { readonly from: readonly string[]; readonly to: string; readonly dryRun: boolean },
): Promise<Rewrite> =>
  parseRewrite(
    await api.writeJson('POST', `${base(scope)}/merge`, {
      from: [...write.from],
      to: write.to,
      dry_run: write.dryRun,
    }),
  );

/** Remember that these spellings name different things; answers the census. */
export const markDistinct = async (
  api: ApiClient,
  scope: ValueScope,
  values: readonly string[],
): Promise<Census> =>
  parseCensus(await api.writeJson('POST', `${base(scope)}/distinct`, { values: [...values] }));

/** The tags join the list with this label, created when none has it. */
export const promoteTags = async (
  api: ApiClient,
  write: { readonly tags: readonly string[]; readonly list: string; readonly dryRun: boolean },
): Promise<Promotion> =>
  parsePromotion(
    await api.writeJson('POST', '/api/tags/promote', {
      tags: [...write.tags],
      list: write.list,
      dry_run: write.dryRun,
    }),
  );
