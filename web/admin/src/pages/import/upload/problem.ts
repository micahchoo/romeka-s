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

/** The problem a thrown error states, with the import's own refusal words. */
export const importProblemOf = (error: unknown): UploadProblem =>
  problemOf(error, (refusal) => {
    switch (refusal.code) {
      case 'importFull':
        return { kind: 'importFull', limit: refusal.limit, used: refusal.used };
      case 'notDraft':
        return { kind: 'importStarted' };
      default:
        return null;
    }
  });
