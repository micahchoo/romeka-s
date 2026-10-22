/**
 * The parsers of the import answers that carry a plan: one import with its
 * plan, and what a pairs upload did with the import after it. The routes
 * load this module, and the plan parser it reads, with the first such answer
 * (`import-routes.ts`), so the plan's tree parsers stay out of the routes'
 * chunk.
 */
import { asCount, list, member, oneOf, strict } from './import-read';
import { parseImportPlan } from './import-plan-tree-read';
import { PAIRS_WHY, ROW_KEYS, rowFields } from './import-view';
import type { ImportDetail, PairsAdded, UnmatchedCell } from './import-view';
import { asString } from './parse';
import type { Parser } from './types';

export const parseImportDetail: Parser<ImportDetail> = (raw, path = []) => {
  const object = strict(raw, path, [...ROW_KEYS, 'plan'], ['progress', 'resumed', 'ranPlan']);
  const detail: ImportDetail = { ...rowFields(object, path), plan: member(object, 'plan', path, parseImportPlan) };
  return object['ranPlan'] === undefined
    ? detail
    : { ...detail, ranPlan: member(object, 'ranPlan', path, parseImportPlan) };
};

const parseUnmatched: Parser<UnmatchedCell> = (raw, path = []) => {
  const object = strict(raw, path, ['row', 'text', 'why']);
  return {
    row: member(object, 'row', path, asString),
    text: member(object, 'text', path, asString),
    why: member(object, 'why', path, oneOf(PAIRS_WHY)),
  };
};

export const parsePairsAdded: Parser<PairsAdded> = (raw, path = []) => {
  const object = strict(raw, path, ['added', 'already', 'unmatched', 'unmatchedTotal', 'import']);
  return {
    added: member(object, 'added', path, asCount),
    already: member(object, 'already', path, asCount),
    unmatched: member(object, 'unmatched', path, list(parseUnmatched)),
    unmatchedTotal: member(object, 'unmatchedTotal', path, asCount),
    import: member(object, 'import', path, parseImportDetail),
  };
};
