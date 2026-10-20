/** What the Run and Check steps share: which statuses to poll, and which refusals are a 409. */
import { ApiError } from '../../api/client';
import type { ImportStatus } from '../../api/imports';

/** The statuses a job is still working through, so the page asks again. */
export const LIVE_STATUSES: ReadonlySet<ImportStatus> = new Set(['running', 'undoing']);

export const isConflict = (error: unknown): boolean => error instanceof ApiError && error.status === 409;
