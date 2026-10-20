/**
 * The undo report (`src/api/import-undo.ts`) against the import rows the
 * undo job produced (`tests/fixtures/import/undo-report-*.json`, written by
 * `crates/omeka-cli/tests/import_undo_wire.rs`): each outcome an undo can end
 * in parses and serialises back to the JSON it came from.
 */
import { describe, expect, it } from 'vitest';
import { parseUndoReport } from '../src/api/import-undo';
import { ParseError } from '../src/api/parse';
import { parseImportRow } from '../src/api/import-view';
import { loadFixture } from './loadFixture';

const NAMES = ['kept', 'forced', 'later-import', 'skipped', 'stopped', 'error'] as const;

const rowOf = (name: (typeof NAMES)[number]) => parseImportRow(loadFixture(`import/undo-report-${name}.json`));

const reportOf = (name: (typeof NAMES)[number]) => {
  const report = rowOf(name).undoReport;
  if (report === null) {
    throw new Error(`${name} has no report`);
  }
  return report;
};

const roundTrip = (value: unknown): unknown => {
  const text: unknown = JSON.stringify(value);
  if (typeof text !== 'string') {
    throw new Error('a report serialises to text');
  }
  const back: unknown = JSON.parse(text);
  return back;
};

describe('the undo report', () => {
  it.each(NAMES)('%s parses and serialises back to the row it came from', (name) => {
    const raw = loadFixture(`import/undo-report-${name}.json`);
    expect(roundTrip(rowOf(name))).toEqual(raw);
  });

  it('lists each kept resource with the reasons the server gave', () => {
    const report = reportOf('kept');
    expect(rowOf('kept').status).toBe('partly_undone');
    const reasons = report.kept.map((kept) => [kept.kind, kept.reasons.map((reason) => reason.reason)]);
    expect(reasons).toContainEqual(['item', ['edited']]);
    expect(reasons).toContainEqual(['media', ['itemKept']]);
    expect(reasons).toContainEqual(['itemSet', ['memberKept']]);
    expect(report.kept.every((kept) => kept.reasons.every((reason) => reason.key.startsWith('server.importUndo')))).toBe(true);
  });

  it('names the later import that changed what is kept', () => {
    const later = reportOf('later-import').kept.flatMap((kept) => kept.reasons).find((reason) => reason.reason === 'laterImport');
    expect(later?.import).toBeGreaterThan(0);
    expect(later?.detail).toContain(String(later?.import));
  });

  it('says what a forced undo deleted anyway', () => {
    const report = reportOf('forced');
    expect(rowOf('forced').status).toBe('undone');
    expect(report.forced).toHaveLength(1);
    expect(report.kept).toEqual([]);
  });

  it('says what was already gone, where it stopped, and what stopped it', () => {
    expect(reportOf('skipped').skipped.map((gone) => gone.kind)).toContain('item');
    const stopped = reportOf('stopped');
    expect(stopped.stopped).toBe(true);
    expect(stopped.progress.done).toBeLessThan(stopped.progress.total);
    const failed = reportOf('error');
    expect(failed.error?.key).toBe('server.importUndoStopped');
    expect(failed.stopped).toBe(false);
  });

  it('refuses a reason it does not know, and an import member on another reason', () => {
    const counts = { items: 0, media: 0, itemSets: 0, templates: 0, properties: 0 };
    const reportKeeping = (reason: Record<string, unknown>): unknown => ({
      progress: { done: 1, total: 1 },
      removed: counts,
      restored: counts,
      kept: [{ kind: 'item', id: 1, reasons: [reason] }],
      skipped: [],
      forced: [],
      stopped: false,
      error: null,
    });
    const edited = { reason: 'edited', key: 'server.importUndoEdited', detail: 'Edited.' };
    expect(parseUndoReport(reportKeeping(edited)).kept).toHaveLength(1);
    expect(() => parseUndoReport(reportKeeping({ ...edited, reason: 'whim' }))).toThrow(ParseError);
    expect(() => parseUndoReport(reportKeeping({ ...edited, import: 3 }))).toThrow(ParseError);
  });
});
