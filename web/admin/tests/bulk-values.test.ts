/**
 * The value edits of a bulk change: the Values form's one change (`values-model.ts`), the list it
 * joins (`withOp`), and the wire (`src/api/bulk-values.ts`) against the answers
 * `crates/omeka-cli/tests/bulk_wire.rs` recorded.
 */
import { describe, expect, it } from 'vitest';
import { NO_EDIT, encodeEdit, parseBulkPreview, parseBulkRun, parseEdit } from '../src/api/bulk';
import type { ValueOp } from '../src/api/bulk-values';
import { withOp } from '../src/api/bulk-values';
import { brandId } from '../src/api/parse';
import { NEW_COMPOSER, opOf } from '../src/pages/items/bulk/values-model';
import { loadFixture } from './loadFixture';

const subject = brandId<'PropertyId'>(3);
const at = { ...NEW_COMPOSER, property: subject };

describe('the Values form', () => {
  it('names no change until the field and the inputs its action needs are there', () => {
    expect(opOf(NEW_COMPOSER)).toBeNull();
    expect(opOf(at)).toBeNull();
    expect(opOf({ ...at, text: '   ' })).toBeNull();
    expect(opOf({ ...at, text: 'Maps', lang: ' ' })).toEqual({
      op: 'add',
      property: subject,
      isPublic: true,
      value: { type: 'literal', text: 'Maps', lang: null },
    });
    expect(opOf({ ...at, kind: 'uri', uri: 'example.org' })).toBeNull();
    expect(opOf({ ...at, kind: 'uri', uri: ' https://example.org ', label: '', isPublic: false })).toEqual({
      op: 'add',
      property: subject,
      isPublic: false,
      value: { type: 'uri', uri: 'https://example.org', label: null },
    });
    expect(opOf({ ...at, kind: 'item', item: '0' })).toBeNull();
    expect(opOf({ ...at, kind: 'item', item: '7' })).toMatchObject({ value: { type: 'item', item: 7 } });
  });

  it('replaces exact text only with another text', () => {
    const replace = { ...at, action: 'replace' as const };
    expect(opOf({ ...replace, from: 'Harbor', to: '' })).toBeNull();
    expect(opOf({ ...replace, from: 'Harbor', to: 'Harbor' })).toBeNull();
    expect(opOf({ ...replace, from: 'Harbor ', to: 'Harbour' })).toEqual({
      op: 'replace',
      property: subject,
      from: 'Harbor ',
      to: 'Harbour',
    });
    expect(opOf({ ...at, action: 'clear' })).toEqual({ op: 'clear', property: subject });
    expect(opOf({ ...at, action: 'visibility', isPublic: false })).toEqual({
      op: 'visibility',
      property: subject,
      isPublic: false,
    });
  });

  it('keeps one rewrite per field and adds a value once', () => {
    const add: ValueOp = { op: 'add', property: subject, isPublic: true, value: { type: 'literal', text: 'Maps', lang: null } };
    const clear: ValueOp = { op: 'clear', property: subject };
    const hide: ValueOp = { op: 'visibility', property: subject, isPublic: false };
    expect(withOp(withOp([add, clear], add), hide)).toEqual([add, hide]);
  });
});

describe('the value wire', () => {
  it('reads a preview line per changed field, with its values before and after', () => {
    const [sample] = parseBulkPreview(loadFixture('bulk/preview-values.json')).samples;
    expect(sample?.values).toEqual([
      {
        property: 3,
        term: 'dcterms:subject',
        label: 'Subject',
        before: [],
        after: [
          { text: 'Maps', lang: 'en', uri: null, resource: null, isPublic: false },
          { text: 'Harbour', lang: null, uri: 'https://example.org/harbour', resource: null, isPublic: true },
          { text: 'Harbour photograph', lang: null, uri: null, resource: 1, isPublic: true },
        ],
      },
    ]);
  });

  it('reads the stored change back as the edit that sent it', () => {
    const run = parseBulkRun(loadFixture('bulk/run-values.json'));
    expect(run.change.values.map((op) => op.op)).toEqual(['add', 'add', 'add']);
    expect(parseEdit(encodeEdit(run.change))).toEqual(run.change);
    expect(encodeEdit({ ...NO_EDIT, values: [{ op: 'clear', property: subject }] })).toEqual({
      kind: 'edit',
      values: [{ op: 'clear', property: 3 }],
    });
  });
});
