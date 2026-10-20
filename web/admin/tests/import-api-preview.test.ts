/**
 * Every preview section (`src/api/import-sections.ts`) against the answer
 * the server's route gave for it (`tests/fixtures/import/preview-*.json`,
 * written by `crates/omeka-server/tests/imports_wire_fixtures.rs`). A parsed
 * answer serialises back to the JSON it came from, so a member the server
 * sends and the client does not know fails here.
 */
import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { SECTIONS, hasSections, parsePreview } from '../src/api/import-sections';
import type { Section } from '../src/api/import-sections';
import { ParseError, asRecord } from '../src/api/parse';
import { loadFixture } from './loadFixture';

const FIXTURES = resolve(dirname(fileURLToPath(import.meta.url)), 'fixtures/import');

const kebab = (name: string): string => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

const roundTrip = (value: unknown): unknown => {
  const text: unknown = JSON.stringify(value);
  if (typeof text !== 'string') {
    throw new Error('a preview serialises to text');
  }
  const back: unknown = JSON.parse(text);
  return back;
};

/** The sections whose server answer does not exist yet (p3-extras). */
const PENDING: readonly Section[] = ['nameWords', 'columnMatches'];

describe('the sections', () => {
  it('are the ones the contract names, and the two p3-extras adds', () => {
    expect([...SECTIONS].sort()).toEqual(
      [
        'anomalies', 'check', 'columnMatches', 'entries', 'flags', 'histogram', 'identical',
        'nameWords', 'orphans', 'records', 'sheets', 'shapes', 'tags', 'unpaired', 'values',
      ].sort(),
    );
  });

  it.each(SECTIONS.filter((section) => !PENDING.includes(section)))(
    '%s parses the server answer and serialises back to it',
    (section) => {
      const raw = loadFixture(`import/preview-${kebab(section)}.json`);
      const preview = parsePreview([section])(raw);
      expect(hasSections(preview, [section])).toBe(true);
      expect(roundTrip(preview)).toEqual(raw);
    },
  );

  it.each(PENDING)('%s is parsed from the server answer once the server writes one', (section) => {
    const name = `preview-${kebab(section)}.json`;
    if (!existsSync(resolve(FIXTURES, name))) {
      // Typed from the contract, not yet from the server: see the module comment.
      return;
    }
    const raw = loadFixture(`import/${name}`);
    expect(roundTrip(parsePreview([section])(raw))).toEqual(raw);
  });

  it('parse the records of a media listing and a blocked check', () => {
    for (const [name, section] of [
      ['preview-records-media.json', 'records'],
      ['preview-check-blocked.json', 'check'],
      ['preview-check-delete.json', 'check'],
    ] as const) {
      const raw = loadFixture(`import/${name}`);
      expect(roundTrip(parsePreview([section])(raw))).toEqual(raw);
    }
  });
});

describe('what the sections say', () => {
  it('check states the digest, the new sets and types, and why a run is blocked', () => {
    const ok = parsePreview(['check'])(loadFixture('import/preview-check.json')).check;
    expect(ok.canRun).toBe(true);
    expect(ok.digest).toHaveLength(64);
    expect(ok.newSets.map((set) => set.title)).toEqual(['Artifacts', 'Photographs', 'Bulk']);
    expect(ok.newTypes[0]?.fields).toContain('dcterms:title');
    const blocked = parsePreview(['check'])(loadFixture('import/preview-check-blocked.json')).check;
    expect(blocked.canRun).toBe(false);
    expect(blocked.blocking[0]).toMatchObject({ kind: 'unknown-term', subject: 'plan', policy: null });
    const deletes = parsePreview(['check'])(loadFixture('import/preview-check-delete.json')).check;
    expect(deletes.action).toBe('delete');
    expect(deletes.deletes).toBe(1);
  });

  it('anomalies cover every variant the derive can state', () => {
    const { anomalies } = parsePreview(['anomalies'])(loadFixture('import/preview-anomalies.json'));
    const names = anomalies.map((anomaly) => Object.keys(anomaly)[0]).sort();
    expect(names).toEqual([
      'decisionOverlap', 'headerNotes', 'largeGroup', 'outsideScope', 'sheetChanged',
      'sheetUnread', 'spanFolders', 'unmappedColumns', 'unusedEntries', 'walk',
    ]);
  });

  it('records say what each does and which flags sit on it', () => {
    const { records } = parsePreview(['records'])(loadFixture('import/preview-records.json'));
    const fates = new Set(records.items.map((record) => (typeof record.fate === 'string' ? record.fate : Object.keys(record.fate)[0])));
    expect(fates).toEqual(new Set(['create', 'left']));
    const own = records.items.find((record) => 'group' in record.origin && 'own' in record.origin.group);
    expect(own?.files).toHaveLength(1);
    expect(records.items.some((record) => record.flags.length > 0)).toBe(true);
  });

  it('shapes carry the facts the records filter takes back', () => {
    const { shapes } = parsePreview(['shapes'])(loadFixture('import/preview-shapes.json'));
    const odd = shapes.groups.items.find((group) => group.odd);
    expect(odd?.facts.files).toBe('7+');
    expect(odd?.samples.length).toBeLessThanOrEqual(3);
  });

  it('a record fate that acts, a flag on a sheet and a left-by flag parse', () => {
    const [record] = parsePreview(['records'])(loadFixture('import/preview-records.json')).records.items;
    const acting = { ...record, fate: { act: 7 }, parent: { existing: 3 }, resolved: { ...record?.resolved, owner: { unknown: 'x@y.z' } } };
    const raw = asRecord(loadFixture('import/preview-records.json'), []);
    const changed = { ...raw, records: { total: 1, page: 1, perPage: 50, items: [acting] } };
    const parsed = parsePreview(['records'])(changed).records.items[0];
    expect(parsed?.fate).toEqual({ act: 7 });
    expect(parsed?.resolved.owner).toEqual({ unknown: 'x@y.z' });
    const flagged = parsePreview(['flags'])({
      ...asRecord(loadFixture('import/preview-flags.json'), []),
      flags: [
        { kind: 'encoding-unsupported', slot: null, policy: null, count: 1, samples: [{ subject: { sheet: 'register' }, record: null, detail: 'Shift-JIS' }] },
      ],
    });
    expect(flagged.flags[0]?.samples[0]?.subject).toEqual({ sheet: 'register' });
  });
});

describe('what a preview parse refuses', () => {
  const raw = loadFixture('import/preview-check.json');
  const rawObject = asRecord(raw, []);

  it('a section the request did not name', () => {
    expect(() => parsePreview(['records'])(raw)).toThrow(ParseError);
    expect(() => parsePreview([])(raw)).toThrow(ParseError);
  });

  it('a requested section the answer leaves out', () => {
    expect(() => parsePreview(['check', 'records'])(raw)).toThrow(ParseError);
  });

  it('a value of the wrong kind, with the path to it', () => {
    const wrong = { ...rawObject, check: { ...asRecord(rawObject['check'], []), canRun: 'yes' } };
    expect(() => parsePreview(['check'])(wrong)).toThrow('$.check.canRun');
  });

  it('a member the client does not know', () => {
    const extra = { ...rawObject, surprise: 1 };
    expect(() => parsePreview(['check'])(extra)).toThrow(ParseError);
  });
});
