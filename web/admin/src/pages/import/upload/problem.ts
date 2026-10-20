/**
 * Why a file did not arrive, in the words the person acts on. One kind per
 * thing they can do about it; the wire refusal maps onto it here, once.
 */
import { ApiError } from '../../../api/client';
import { refusalOf } from '../../../api/import-uploads';

export type UploadProblem =
  | { readonly kind: 'fileTooLarge'; readonly limit: number }
  | { readonly kind: 'importFull'; readonly limit: number; readonly used: number }
  | { readonly kind: 'badName' }
  /** The import has run: no file can be added any more. */
  | { readonly kind: 'importStarted' }
  | { readonly kind: 'notAllowed' }
  /** The server no longer holds the bytes sent so far. */
  | { readonly kind: 'gone' }
  | { readonly kind: 'zipRefused'; readonly reason: string }
  | { readonly kind: 'connection' }
  | { readonly kind: 'server'; readonly message: string };

/** The problems that stop every file of the import, not just one. */
export const closesImport = (problem: UploadProblem): boolean =>
  problem.kind === 'importStarted' || problem.kind === 'notAllowed';

/** The problem a thrown error states. `uploadInProgress` and `offsetConflict` are recovered before this. */
export const problemOf = (error: unknown): UploadProblem => {
  const refusal = refusalOf(error);
  switch (refusal?.code) {
    case 'fileTooLarge':
      return { kind: 'fileTooLarge', limit: refusal.limit };
    case 'importFull':
      return { kind: 'importFull', limit: refusal.limit, used: refusal.used };
    case 'uploadPath':
      return { kind: 'badName' };
    case 'notDraft':
      return { kind: 'importStarted' };
    case 'forbidden':
      return { kind: 'notAllowed' };
    case 'gone':
      return { kind: 'gone' };
    case 'uploadInProgress':
    case 'offsetConflict':
    case undefined:
      break;
  }
  if (error instanceof ApiError) {
    return { kind: 'server', message: error.message };
  }
  if (error instanceof TypeError) {
    return { kind: 'connection' };
  }
  return { kind: 'server', message: error instanceof Error ? error.message : String(error) };
};
