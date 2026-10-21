/**
 * The destination an upload queue sends to. An import's staging area and a
 * person's staged uploads differ in their collection path, in whether a whole
 * zip is unpacked, and in which problems stop every file; everything else is
 * the same queue.
 *
 * The limits are read with the listing, because an import's listing already
 * answers them in the same response; a destination whose listing states none
 * supplies `limits()`. The upload's words for a refusal come from the target
 * too, so a screen renders a problem without knowing which route said it.
 */
import type { UploadProblem } from './problem';
import type { StoredUpload, UploadLimit } from './tus';

/** What a destination holds now. */
export interface UploadListing {
  readonly uploads: readonly StoredUpload[];
  readonly used: number;
  readonly limit: UploadLimit | null;
}

/** One PATCH's answer: where the server holds it now, and the unpack job it started. */
export interface SentChunk {
  readonly offset: number;
  readonly unpack: Unpack | null;
}

export type UnpackState =
  | { readonly kind: 'running' }
  | { readonly kind: 'done' }
  | { readonly kind: 'failed'; readonly problem: UploadProblem };

/** The server job that unpacks a whole zip, for a destination that unpacks. */
export interface Unpack {
  check(): Promise<UnpackState>;
}

export interface UploadTarget {
  /** The uploads the server holds and the limits it applies. */
  list(): Promise<UploadListing>;
  /** The limits, for a destination whose listing states none. */
  limits?(): Promise<UploadLimit | null>;
  create(path: string, length: number, mediaType: string | null): Promise<string>;
  offset(upload: string): Promise<{ readonly offset: number; readonly length: number }>;
  append(upload: string, offset: number, bytes: Blob, signal?: AbortSignal): Promise<SentChunk>;
  remove(upload: string): Promise<void>;
  /** The problem a thrown error states, in this destination's words. */
  problemOf(error: unknown): UploadProblem;
  /** True when this problem stops every file of the destination, not just one. */
  closes(problem: UploadProblem): boolean;
}
