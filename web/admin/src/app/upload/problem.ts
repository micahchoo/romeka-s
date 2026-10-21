/**
 * Why a file did not arrive, in the words the person acts on. One kind per
 * thing they can do about it.
 *
 * The union is shared by every destination, so an import's words for a closed
 * import and a refused zip live here too. Their refusals are classified by the
 * import's own target (`api/import-uploads.ts`), not by this module: a
 * destination names what its own routes answer.
 */
import { ApiError } from '../../api/client';
import type { UploadRefusal } from './tus';
import { refusalOf } from './tus';

export type UploadProblem =
  | { readonly kind: 'fileTooLarge'; readonly limit: number }
  | { readonly kind: 'importFull'; readonly limit: number; readonly used: number }
  | { readonly kind: 'badName' }
  /** The destination has run: no file can be added any more. */
  | { readonly kind: 'importStarted' }
  | { readonly kind: 'notAllowed' }
  /** The server no longer holds the bytes sent so far. */
  | { readonly kind: 'gone' }
  | { readonly kind: 'zipRefused'; readonly reason: string }
  | { readonly kind: 'connection' }
  | { readonly kind: 'server'; readonly message: string };

/** A destination's word for a refusal the shared table does not name. */
export type RefusalProblems = (refusal: UploadRefusal) => UploadProblem | null;

const sharedProblem = (refusal: UploadRefusal): UploadProblem | null => {
  switch (refusal.code) {
    case 'fileTooLarge':
      return { kind: 'fileTooLarge', limit: refusal.limit };
    case 'uploadPath':
      return { kind: 'badName' };
    case 'forbidden':
      return { kind: 'notAllowed' };
    case 'gone':
      return { kind: 'gone' };
    case 'importFull':
    case 'notDraft':
    case 'uploadInProgress':
    case 'offsetConflict':
      return null;
  }
};

/** The problem a thrown error states. `uploadInProgress` and `offsetConflict` are recovered before this. */
export const problemOf = (error: unknown, extra: RefusalProblems = () => null): UploadProblem => {
  const refusal = refusalOf(error);
  if (refusal !== null) {
    const known = sharedProblem(refusal) ?? extra(refusal);
    if (known !== null) {
      return known;
    }
  }
  if (error instanceof ApiError) {
    return { kind: 'server', message: error.message };
  }
  if (error instanceof TypeError) {
    return { kind: 'connection' };
  }
  return { kind: 'server', message: error instanceof Error ? error.message : String(error) };
};
