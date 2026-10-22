/**
 * The 409 an import route answers: the error document plus `code`, the name
 * the contract gives it, and the members the admin needs to act on it
 * (`docs/import-v2.md`, "Wire details"; `ImportError` in
 * `crates/omeka-server/src/imports/wire.rs`).
 *
 * `ImportApi` throws an `ImportConflictError` for a 409 whose code and members
 * it knows, and a plain `ApiError` for anything else, so a page branches with
 * `conflictOf(error)` and a switch over `code` that `tsc` checks for
 * completeness. The 400 codes of a list round trip that name the wrong series
 * or list are a `SeriesRefusalError` the same way (`seriesRefusalOf`).
 */
import { ApiError } from './client';
import type { ApiErrors } from './types';
import { parseFlag } from './import-model';
import type { Flag } from './import-model';
import { IMPORT_STATUSES } from './import-view';
import type { ImportStatus } from './import-view';
import type { LevelId } from './import-plan';
import { asCount, list, member, oneOf, strict } from './import-read';
import { ParseError, asString, isRecord } from './parse';

export const CONFLICT_CODES = [
  'stale',
  'notDraft',
  'v1Plan',
  'typeExists',
  'changed',
  'blocked',
  'nothing',
  'confirm',
  'busy',
  'cannotRun',
  'cannotUndo',
  'notRunning',
] as const;
export type ConflictCode = (typeof CONFLICT_CODES)[number];

export type ImportConflict =
  /** Someone saved after this read: `revision` is the stored one. */
  | { readonly code: 'stale'; readonly revision: number }
  /** The plan of an import that has started cannot change; copy it. */
  | { readonly code: 'notDraft'; readonly status: ImportStatus }
  | { readonly code: 'v1Plan' }
  /** A new type's label is a template's already. */
  | { readonly code: 'typeExists'; readonly label: string }
  /** The files or the plan changed since the check: `digest` is the fresh one, or `revision` the stored plan's. */
  | { readonly code: 'changed'; readonly digest: string | null; readonly revision: number | null }
  /** A blocking flag stands. */
  | { readonly code: 'blocked'; readonly blocking: readonly Flag[] }
  | { readonly code: 'nothing' }
  /** A delete needs `confirm` equal to `deletes`. */
  | { readonly code: 'confirm'; readonly deletes: number }
  /** Another import runs or is undone (`importId`), or this one is scanning. */
  | { readonly code: 'busy'; readonly importId: number | null }
  | { readonly code: 'cannotRun'; readonly status: ImportStatus }
  | { readonly code: 'cannotUndo'; readonly status: ImportStatus }
  | { readonly code: 'notRunning'; readonly status: ImportStatus };

const BASE = ['errors', 'code'];
const status = oneOf(IMPORT_STATUSES);

const readConflict = (body: Record<string, unknown>, code: ConflictCode): ImportConflict => {
  const members = (...keys: string[]): Record<string, unknown> => strict(body, [], [...BASE, ...keys]);
  switch (code) {
    case 'stale': {
      const object = members('revision');
      return { code, revision: member(object, 'revision', [], asCount) };
    }
    case 'notDraft':
    case 'cannotRun':
    case 'cannotUndo':
    case 'notRunning': {
      const object = members('status');
      return { code, status: member(object, 'status', [], status) };
    }
    case 'v1Plan':
    case 'nothing':
      members();
      return { code };
    case 'typeExists': {
      const object = members('label');
      return { code, label: member(object, 'label', [], asString) };
    }
    case 'changed': {
      const object = strict(body, [], BASE, ['digest', 'revision']);
      return {
        code,
        digest: object['digest'] === undefined ? null : member(object, 'digest', [], asString),
        revision: object['revision'] === undefined ? null : member(object, 'revision', [], asCount),
      };
    }
    case 'blocked': {
      const object = members('blocking');
      return { code, blocking: member(object, 'blocking', [], list(parseFlag)) };
    }
    case 'confirm': {
      const object = members('deletes');
      return { code, deletes: member(object, 'deletes', [], asCount) };
    }
    case 'busy': {
      const object = strict(body, [], BASE, ['importId']);
      return { code, importId: object['importId'] === undefined ? null : member(object, 'importId', [], asCount) };
    }
  }
};

/**
 * The conflict a 409 body states, or `null` for a body with no known code or
 * members the client does not know (a plain error then).
 */
export const parseConflict = (body: unknown): ImportConflict | null => {
  if (!isRecord(body)) {
    return null;
  }
  const code = CONFLICT_CODES.find((candidate) => candidate === body['code']);
  if (code === undefined) {
    return null;
  }
  try {
    return readConflict(body, code);
  } catch (error) {
    if (error instanceof ParseError) {
      return null;
    }
    throw error;
  }
};

export class ImportConflictError extends ApiError {
  readonly conflict: ImportConflict;

  constructor(
    conflict: ImportConflict,
    errors: ApiErrors | null,
    message: string,
    sourceErrors: ApiErrors | null,
  ) {
    super(409, errors, message, sourceErrors);
    this.name = 'ImportConflictError';
    this.conflict = conflict;
  }
}

/** The typed conflict of a thrown error, or `null` when it is not one. */
export const conflictOf = (error: unknown): ImportConflict | null =>
  error instanceof ImportConflictError ? error.conflict : null;

export const SERIES_CODES = ['seriesNeeded', 'unknownSeries', 'unknownList'] as const;
export type SeriesCode = (typeof SERIES_CODES)[number];

/**
 * The 400 a list round trip (`template.csv`, `POST …/sheets`) answers in a
 * plan of several series: `?series=` was left out (`seriesNeeded`) or names
 * no series (`unknownSeries`). `POST …/sheets` also answers it when
 * `?replace=` names no list the person uploaded to the series
 * (`unknownList`). `series` is every series id of the plan, so the admin can
 * ask which one the list is for.
 */
export interface SeriesRefusal {
  readonly code: SeriesCode;
  readonly series: readonly LevelId[];
}

/** The refusal a 400 body states, or `null` for any other body. */
export const parseSeriesRefusal = (body: unknown): SeriesRefusal | null => {
  if (!isRecord(body)) {
    return null;
  }
  const code = SERIES_CODES.find((candidate) => candidate === body['code']);
  if (code === undefined) {
    return null;
  }
  try {
    return { code, series: member(strict(body, [], [...BASE, 'series']), 'series', [], list(asString)) };
  } catch (error) {
    if (error instanceof ParseError) {
      return null;
    }
    throw error;
  }
};

export class SeriesRefusalError extends ApiError {
  readonly refusal: SeriesRefusal;

  constructor(refusal: SeriesRefusal, errors: ApiErrors | null, message: string, sourceErrors: ApiErrors | null) {
    super(400, errors, message, sourceErrors);
    this.name = 'SeriesRefusalError';
    this.refusal = refusal;
  }
}

/** The typed series refusal of a thrown error, or `null` when it is not one. */
export const seriesRefusalOf = (error: unknown): SeriesRefusal | null =>
  error instanceof SeriesRefusalError ? error.refusal : null;
