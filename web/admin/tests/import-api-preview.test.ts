/**
 * Every preview section (`src/api/import-sections.ts`) against the answer
 * the server's route gave for it (`tests/fixtures/import/preview-*.json`,
 * written by `crates/omeka-server/tests/imports_wire_fixtures.rs`). A parsed
 * answer serialises back to the JSON it came from, so a member the server
 * sends and the client does not know fails here.
 */
import { describe, expect, it } from 'vitest';
import { TAG_IDS } from '../src/api/import-plan';
import { en as importMessages } from '../src/i18n/namespaces/import';
import { SECTIONS, hasSections, parsePreview } from '../src/api/import-sections';
import { ParseError, asRecord } from '../src/api/parse';
import { loadFixture } from './loadFixture';

const kebab = (name: string): string => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

const roundTrip = (value: unknown): unknown => {
  const text: unknown = JSON.stringify(value);
  if (typeof text !== 'string') {
    throw new Error('a preview serialises to text');
  }
  const back: unknown = JSON.parse(text);
  return back;
};

describe('the sections', () => {
  it('are the ones the contract names', () => {
    expect([...SECTIONS].sort()).toEqual(
      [
        'anomalies', 'check', 'columnMatches', 'entries', 'flags', 'histogram', 'identical',
        'listCandidates', 'nameWords', 'orphans', 'records', 'sheets', 'shapes', 'tags', 'unpaired', 'values',
      ].sort(),
    );
  });

  it.each(SECTIONS)(
    '%s parses the server answer and serialises back to it',
    (section) => {
      const raw = loadFixture(`import/preview-${kebab(section)}.json`);
      const preview = parsePreview([section])(raw);
      expect(hasSections(preview, [section])).toBe(true);
      expect(roundTrip(preview)).toEqual(raw);
    },
  );

  it('parse a workbook list with its sheet names, and the 25 tags with their kinds', () => {
    const raw = loadFixture('import/preview-sheets-workbook.json');
    const { sheets } = parsePreview(['sheets'])(raw);
    expect(roundTrip(parsePreview(['sheets'])(raw))).toEqual(raw);
    expect(sheets.map((view) => view.read?.workbook ?? null)).toEqual([
      null,
      { sheet: 'Register', sheets: ['Register', 'Notes'] },
    ]);
    const { tags } = parsePreview(['tags'])(loadFixture('import/preview-tags.json'));
    expect(tags).toHaveLength(25);
    expect([...new Set(tags.map((tag) => tag.kind))]).toEqual(['exif', 'iptc', 'xmp', 'file']);
    expect(tags.map((tag) => tag.tag)).toEqual(TAG_IDS);
  });

  it('every tag and every tag kind has a label in the import namespace', () => {
    const keys = new Set(Object.keys(importMessages));
    const missing = [...TAG_IDS.map((id) => `tag.${id}`), ...['exif', 'iptc', 'xmp', 'file'].map((kind) => `tag.kind.${kind}`)]
      .filter((key) => !keys.has(key));
    expect(missing).toEqual([]);
  });

  it('parse the records of a media listing and a blocked check', () => {
    for (const [name, section] of [
      ['preview-records-media.json', 'records'],
      ['preview-check-blocked.json', 'check'],
      ['preview-check-delete.json', 'check'],
      ['preview-records-no-row.json', 'records'],
    ] as const) {
      const raw = loadFixture(`import/${name}`);
      expect(roundTrip(parsePreview([section])(raw))).toEqual(raw);
    }
  });
});

describe('what the sections say', () => {
  it('nameWords splits the requested name and counts the words at each place', () => {
    const { nameWords } = parsePreview(['nameWords'])(loadFixture('import/preview-name-words.json'));
    expect(nameWords.parts).toEqual(['BHC', '006', 'AF', '001', 'front', 'jpg']);
    expect(nameWords.places).toHaveLength(nameWords.parts.length);
    expect(nameWords.names).toBe(18);
    expect(nameWords.places[0]?.items).toEqual([{ text: 'BHC', count: 18 }]);
    expect(nameWords.places[2]?.items.map((word) => word.text)).toEqual(['AF', 'BK', 'SC']);
  });

  it('columnMatches count the cells that equal a group key or a file name', () => {
    const { columnMatches } = parsePreview(['columnMatches'])(loadFixture('import/preview-column-matches.json'));
    const register = columnMatches.find((sheet) => sheet.sheet === 'register');
    expect(register?.rows).toBeGreaterThan(0);
    const keyed = register?.columns.filter((column) => column.groupKeys > 0) ?? [];
    expect(keyed.map((column) => column.header)).toEqual(['Id']);
  });

  it('listCandidates names each list with its best column, and a list that did not read with its error', () => {
    const { listCandidates } = parsePreview(['listCandidates'])(loadFixture('import/preview-list-candidates.json'));
    expect(listCandidates.map((one) => [one.file, one.inPlan, one.read, one.best?.on, one.best?.header])).toEqual([
      ['register.csv', 'register', 'ok', 'identifier', 'Id'],
    ]);
    const answer = {
      sources: [],
      pattern: { groups: [] },
      listCandidates: [
        {
          source: 's1', file: 'AF.xlsx', inPlan: null, read: 'ok', rows: 8, columns: 2,
          best: { index: 0, header: 'File', on: 'fileName', paired: 8, pairedNormalized: 8, empty: 0 },
          grain: { groupsWithManyRows: 4, rowsNamingOtherFilesOfAGroup: 4 },
          overlaps: [{ source: 's1', file: 'old/AF.csv', sharedTargets: 3 }],
        },
        {
          source: 's1', file: 'broken.xlsx', inPlan: null, read: { error: 'the workbook cannot be read: no zip' },
          rows: 0, columns: 0, best: null, grain: null, overlaps: [],
        },
      ],
    };
    const parsed = parsePreview(['listCandidates'])(answer).listCandidates;
    expect(parsed[0]?.grain?.rowsNamingOtherFilesOfAGroup).toBe(4);
    expect(parsed[1]?.read).toEqual({ error: 'the workbook cannot be read: no zip' });
    expect(roundTrip(parsed)).toEqual(answer.listCandidates);
    expect(() => parsePreview(['listCandidates'])({ ...answer, listCandidates: [{ ...answer.listCandidates[1], read: 'failed' }] })).toThrow(ParseError);
  });

  it('entries and records say why a file or an item is left out, copies and rowless items included', () => {
    const entries = parsePreview(['entries'])(loadFixture('import/preview-entries.json')).entries;
    expect(entries.items.map((entry) => entry.left)).toContain('identical');
    const records = parsePreview(['records'])(loadFixture('import/preview-records-no-row.json')).records;
    const fates = JSON.stringify(records.items.map((record) => record.fate));
    expect(fates).toContain('noRow');
  });

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
