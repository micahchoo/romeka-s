/**
 * The `sheets` and `tags` sections of a preview: how each plan sheet read, and
 * the tags the first files offer (`crates/omeka-api/src/import2/preview/evidence.rs`).
 */
import { asCount, asFlag, list, member, nullable, oneOf, strict } from './import-read';
import { DELIMITERS, ENCODINGS, TAG_IDS } from './import-plan';
import type { Delimiter, Encoding, TagId } from './import-plan';
import { asNumber, asString } from './parse';
import type { Parser } from './types';

const text = (object: Record<string, unknown>, key: string, path: readonly (string | number)[]): string =>
  member(object, key, path, asString);
const count = (object: Record<string, unknown>, key: string, path: readonly (string | number)[]): number =>
  member(object, key, path, asCount);
const maybeText = nullable(asString);

export interface EncodingRead {
  readonly encoding: Encoding;
  /** For `auto`, how sure the guess is, 0 to 1. */
  readonly confidence: number | null;
}

const parseEncodingRead: Parser<EncodingRead> = (raw, path = []) => {
  const object = strict(raw, path, ['encoding', 'confidence']);
  return {
    encoding: member(object, 'encoding', path, oneOf(ENCODINGS)),
    confidence: member(object, 'confidence', path, nullable(asNumber)),
  };
};

export interface ColumnView {
  readonly index: number;
  readonly header: string;
  readonly mappable: boolean;
}

export interface SheetRead {
  readonly hash: string;
  readonly encoding: EncodingRead;
  readonly delimiter: Delimiter;
  readonly delimiterGuessed: boolean;
  readonly headerLine: number;
  readonly rows: number;
  readonly cellsCount: number;
  readonly blankLines: number;
  readonly cellsWithSpaces: number;
  readonly cellsSuspect: number;
  readonly columns: readonly ColumnView[];
  readonly notes: readonly string[];
}

const parseColumnView: Parser<ColumnView> = (raw, path = []) => {
  const object = strict(raw, path, ['index', 'header', 'mappable']);
  return { index: count(object, 'index', path), header: text(object, 'header', path), mappable: member(object, 'mappable', path, asFlag) };
};

const parseSheetRead: Parser<SheetRead> = (raw, path = []) => {
  const object = strict(raw, path, [
    'hash',
    'encoding',
    'delimiter',
    'delimiterGuessed',
    'headerLine',
    'rows',
    'cellsCount',
    'blankLines',
    'cellsWithSpaces',
    'cellsSuspect',
    'columns',
    'notes',
  ]);
  return {
    hash: text(object, 'hash', path),
    encoding: member(object, 'encoding', path, parseEncodingRead),
    delimiter: member(object, 'delimiter', path, oneOf(DELIMITERS)),
    delimiterGuessed: member(object, 'delimiterGuessed', path, asFlag),
    headerLine: count(object, 'headerLine', path),
    rows: count(object, 'rows', path),
    cellsCount: count(object, 'cellsCount', path),
    blankLines: count(object, 'blankLines', path),
    cellsWithSpaces: count(object, 'cellsWithSpaces', path),
    cellsSuspect: count(object, 'cellsSuspect', path),
    columns: member(object, 'columns', path, list(parseColumnView)),
    notes: member(object, 'notes', path, list(asString)),
  };
};

/** What auto-detection would read, so the admin can offer it. */
export interface SheetGuess {
  readonly encoding: EncodingRead | null;
  /** The detected encoding no decoder exists for, such as Shift-JIS. */
  readonly unsupported: string | null;
  readonly delimiter: Delimiter;
  readonly headerLine: number;
}

export interface SheetView {
  readonly id: string;
  readonly source: string;
  readonly file: string;
  /** `false` when the file is missing from its source. */
  readonly found: boolean;
  readonly error: string | null;
  readonly read: SheetRead | null;
  readonly guess: SheetGuess | null;
  /** The first raw lines, for choosing the header line. */
  readonly lines: readonly string[];
}

const parseSheetGuess: Parser<SheetGuess> = (raw, path = []) => {
  const object = strict(raw, path, ['encoding', 'unsupported', 'delimiter', 'headerLine']);
  return {
    encoding: member(object, 'encoding', path, nullable(parseEncodingRead)),
    unsupported: member(object, 'unsupported', path, maybeText),
    delimiter: member(object, 'delimiter', path, oneOf(DELIMITERS)),
    headerLine: count(object, 'headerLine', path),
  };
};

export const parseSheetView: Parser<SheetView> = (raw, path = []) => {
  const object = strict(raw, path, ['id', 'source', 'file', 'found', 'error', 'read', 'guess', 'lines']);
  return {
    id: text(object, 'id', path),
    source: text(object, 'source', path),
    file: text(object, 'file', path),
    found: member(object, 'found', path, asFlag),
    error: member(object, 'error', path, maybeText),
    read: member(object, 'read', path, nullable(parseSheetRead)),
    guess: member(object, 'guess', path, nullable(parseSheetGuess)),
    lines: member(object, 'lines', path, list(asString)),
  };
};

export interface TagSample {
  readonly source: string;
  readonly path: string;
  readonly value: string;
}

export interface TagView {
  readonly tag: TagId;
  /** `exif` is read from inside the file; `file` is a fact about it. */
  readonly kind: 'exif' | 'file';
  readonly samples: readonly TagSample[];
}

export const parseTagView: Parser<TagView> = (raw, path = []) => {
  const object = strict(raw, path, ['tag', 'kind', 'samples']);
  return {
    tag: member(object, 'tag', path, oneOf(TAG_IDS)),
    kind: member(object, 'kind', path, oneOf<'exif' | 'file'>(['exif', 'file'])),
    samples: member(
      object,
      'samples',
      path,
      list((value, at) => {
        const sample = strict(value, at, ['source', 'path', 'value']);
        return { source: text(sample, 'source', at), path: text(sample, 'path', at), value: text(sample, 'value', at) };
      }),
    ),
  };
};

