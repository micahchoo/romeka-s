/**
 * The refusals only an import answers: a draft that has run, its total, and a
 * zip the server would not unpack. Every other problem is shared
 * (`app/upload/problem.ts`); this module adds the import's own words and the
 * two that stop every file of the import.
 */
import { problemOf } from '../../../app/upload/problem';
import type { UploadProblem } from '../../../app/upload/problem';

export type { UploadProblem };

/** The problems that stop every file of the import, not just one. */
export const closesImport = (problem: UploadProblem): boolean =>
  problem.kind === 'importStarted' || problem.kind === 'notAllowed';

/** A whole number the import's refusal body carries, or null for anything else. */
const whole = (value: unknown): number | null =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;

/** The problem a thrown error states, with the import's own refusal words. */
export const importProblemOf = (error: unknown): UploadProblem =>
  problemOf(error, (refusal) => {
    // `importFull` and `notDraft` are the import's wire codes, so the shared
    // reader hands them over as `other` with their names and bodies.
    if (refusal.code !== 'other') {
      return null;
    }
    if (refusal.wire === 'notDraft') {
      return { kind: 'importStarted' };
    }
    if (refusal.wire === 'importFull') {
      const limit = whole(refusal.body['limit']);
      const used = whole(refusal.body['used']);
      return limit === null || used === null ? null : { kind: 'importFull', limit, used };
    }
    return null;
  });
