/**
 * What the jump-to box finds for a text. `#<id>` is one `/api/resources/<id>` read. Any other
 * text runs the steps of `STEPS` in order and stops at the first that matches anything: the
 * exact identifier, then the exact title, then a title that contains the text. Each step is one
 * property search per kind (items, item sets, media), so every answer is the caller's own and a
 * hidden record answers like a missing one.
 */
import { ApiError, api } from '../api/client';
import { LINK_KINDS, parseRecordSummary, placesApi } from '../api/places';
import type { LinkKind, RecordSummary } from '../api/places';
import { ParseError, brandId } from '../api/parse';

/** The most records the box lists. */
export const MOST_SHOWN = 10;

export type JumpAnswer =
  | { readonly kind: 'open'; readonly record: RecordSummary }
  | { readonly kind: 'choose'; readonly records: readonly RecordSummary[]; readonly total: number }
  | { readonly kind: 'none' };

interface Step {
  readonly term: 'dcterms:identifier' | 'dcterms:title';
  readonly type: 'eq' | 'in';
}

const STEPS: readonly Step[] = [
  { term: 'dcterms:identifier', type: 'eq' },
  { term: 'dcterms:title', type: 'eq' },
  { term: 'dcterms:title', type: 'in' },
];

const ID = /^#\s*([1-9]\d{0,14})$/;

const NONE: JumpAnswer = { kind: 'none' };

const byId = async (id: number): Promise<JumpAnswer> => {
  try {
    return { kind: 'open', record: await placesApi.record(brandId<'ResourceId'>(id)) };
  } catch (error) {
    // 404 missing, 403 a kind the caller may not read; a value annotation is no record to open.
    if ((error instanceof ApiError && (error.status === 404 || error.status === 403)) || error instanceof ParseError) {
      return NONE;
    }
    throw error;
  }
};

const searchKind = (kind: LinkKind, step: Step, text: string) =>
  api.searchPage(
    kind,
    {
      'property[0][property]': step.term,
      'property[0][type]': step.type,
      'property[0][text]': text,
      perPage: MOST_SHOWN,
      sortBy: 'title',
      sortOrder: 'asc',
    },
    parseRecordSummary,
  );

const byStep = async (step: Step, text: string): Promise<JumpAnswer> => {
  const pages = await Promise.all(LINK_KINDS.map((kind) => searchKind(kind, step, text)));
  const total = pages.reduce((sum, page) => sum + page.total, 0);
  const records = pages.flatMap((page) => page.items);
  const [only] = records;
  if (total === 0) {
    return NONE;
  }
  return total === 1 && only !== undefined
    ? { kind: 'open', record: only }
    : { kind: 'choose', records: records.slice(0, MOST_SHOWN), total };
};

export const findRecord = async (text: string): Promise<JumpAnswer> => {
  const id = ID.exec(text)?.[1];
  if (id !== undefined) {
    return byId(Number(id));
  }
  for (const step of STEPS) {
    const answer = await byStep(step, text);
    if (answer.kind !== 'none') {
      return answer;
    }
  }
  return NONE;
};
