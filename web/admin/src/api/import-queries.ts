/**
 * What a preview request asks of its sections: the filters on `records` and
 * `entries` and the pages of the other lists (`crates/omeka-api/src/import2/preview/query.rs`
 * and its siblings). A filter left out does not narrow. Every body these types
 * can build is one the server takes (`fixtures/import/requests.json`).
 */
import type { FlagKind, ImportPlan, RecordKind, SlotName } from './import-plan';
import type { Bucket, ShapeFacts } from './import-sections';

export interface PageQuery {
  readonly page?: number;
  readonly perPage?: number;
  /** Compared as titles: trim, NFC, case fold. */
  readonly text?: string;
}

export type SetFilter = { readonly existing: number } | { readonly record: number } | { readonly new: number };
export type TemplateFilter = { readonly existing: number } | { readonly new: number } | 'none';
export type Unused = 'unmatched' | 'decision' | 'flag' | 'parent' | 'noFiles' | 'identical' | 'noRow' | 'noRecord';
export const UNUSED_REASONS: readonly Unused[] = ['unmatched', 'decision', 'flag', 'parent', 'noFiles', 'identical', 'noRow', 'noRecord'];

export interface RecordsQuery {
  readonly kind?: RecordKind;
  readonly page?: number;
  readonly perPage?: number;
  readonly flag?: FlagKind;
  /** With `flag`, its flags on this slot; alone, records with a value in it. */
  readonly slot?: SlotName;
  readonly text?: string;
  readonly shape?: ShapeFacts;
  /** A file in this folder or below it. */
  readonly folder?: string;
  readonly set?: SetFilter;
  readonly template?: TemplateFilter;
  readonly files?: Bucket;
  readonly left?: boolean;
}

export interface EntriesQuery {
  readonly page?: number;
  readonly perPage?: number;
  readonly flag?: FlagKind;
  readonly text?: string;
  readonly used?: boolean;
  readonly reason?: Unused;
  readonly folder?: string;
  /** Sheet rows whose record has files (`true`) or none (`false`); files are then never in the answer. */
  readonly paired?: boolean;
  /** Only files, or only list rows. */
  readonly kind?: 'file' | 'row';
  /** Only the rows of this sheet; files are then never in the answer. */
  readonly sheet?: string;
}

export interface UnpairedQuery {
  readonly rows?: PageQuery;
  readonly files?: PageQuery;
}

export interface ValuesQuery {
  readonly kind?: RecordKind;
  readonly page?: number;
  readonly perPage?: number;
}

/** The words at each place of the name of `path`; without a path the answer is empty. */
export interface NameWordsQuery {
  /** The example's path in its source: only its name is split. */
  readonly path?: string;
  readonly page?: number;
  readonly perPage?: number;
  /** Words holding this text, compared as titles. */
  readonly text?: string;
}

export interface PreviewOptions {
  /** A plan to preview in place of the stored one; it is not saved. */
  readonly plan?: ImportPlan;
  readonly records?: RecordsQuery;
  readonly entries?: EntriesQuery;
  readonly shapes?: PageQuery;
  readonly identical?: PageQuery;
  readonly unpaired?: UnpairedQuery;
  readonly values?: ValuesQuery;
  readonly nameWords?: NameWordsQuery;
  /** Draws the sample records of `shapes` and `flags`; the same seed draws the same ones. */
  readonly seed?: number;
}

