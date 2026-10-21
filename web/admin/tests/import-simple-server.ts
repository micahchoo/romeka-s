/**
 * A stand-in for the routes the Simple pass calls, over one scenario
 * (`import-simple-scenarios.ts`). It answers each preview from the plan the
 * request sends, or the plan last saved, as the server does: the groups
 * follow the marks the pattern holds, the list reads only on its header line, the item type values
 * follow the item type rule, and the answer holds exactly the sections asked
 * for. The shapes are the P4-api fixtures', so the pages run through the
 * real client and its parsers.
 */
import { asRecord, isRecord } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import type { Scenario, ScenarioGroup, ScenarioList } from './import-simple-scenarios';
import { jsonResponse, listResponse, route } from './stub';
import type { Handler } from './stub';

const fixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);

export const PATH = '/api/romeka/imports/1';

export interface SimpleServer {
  readonly handler: Handler;
  /** The plan of each PUT, in order. */
  readonly saved: Record<string, unknown>[];
  /** The body of each run request. */
  readonly runs: Record<string, unknown>[];
}

interface Page {
  readonly page: number;
  readonly perPage: number;
  readonly text: string | null;
}

const pageOf = (raw: unknown, fallback = 50): Page => {
  const query = isRecord(raw) ? raw : {};
  return {
    page: typeof query['page'] === 'number' ? query['page'] : 1,
    perPage: Math.min(500, typeof query['perPage'] === 'number' ? query['perPage'] : fallback),
    text: typeof query['text'] === 'string' ? query['text'] : null,
  };
};

const paged = (rows: readonly unknown[], query: Page): Record<string, unknown> => ({
  total: rows.length,
  page: query.page,
  perPage: query.perPage,
  items: rows.slice((query.page - 1) * query.perPage, query.page * query.perPage),
});

const BUCKET_FILES: Readonly<Record<string, number>> = { '0': 0, '1': 1, '2': 2, '3': 3, '4-6': 5, '7+': 200 };

const recordTemplate = (): Record<string, unknown> => {
  const records = asRecord(fixture('preview-records.json')['records'], []);
  const items = records['items'];
  if (!Array.isArray(items) || !isRecord(items[0])) {
    throw new Error('preview-records.json has no record');
  }
  return items[0];
};

/** The marks the plan's pattern holds: the example's, while the pattern has the `part` group they write. */
const marksOf = (plan: Record<string, unknown>): number => {
  const grouping = isRecord(plan['grouping']) ? plan['grouping'] : {};
  const pattern = typeof grouping['pattern'] === 'string' ? grouping['pattern'] : '';
  const example = isRecord(plan['example']) ? plan['example'] : {};
  return pattern.includes('(?<part>') && Array.isArray(example['marks']) ? example['marks'].length : 0;
};

const shownGroups = (scenario: Scenario, marks: number): readonly ScenarioGroup[] =>
  scenario.groups.filter((group) => group.from <= marks && (group.until === undefined || marks <= group.until));

/** A list row joins only once the plan reads the list, so before that no group has one. */
const factsOf = (group: ScenarioGroup, marks: number, listed: boolean): Record<string, unknown> => ({
  files: group.files,
  folders: [group.folder],
  media: [group.media],
  origin: 'pattern',
  row: listed && group.row,
  // Every mark writes the one `part` group (`pages/import/example.ts`), so page numbers are parts too.
  parts: marks > 0 ? [...(group.parts ?? []), ...(group.pages ?? [])] : null,
  pages: null,
});

/** Groups whose facts are equal are one group, as the server counts them. */
const merged = (groups: readonly ScenarioGroup[], marks: number, listed: boolean): readonly ScenarioGroup[] => {
  const by = new Map<string, ScenarioGroup>();
  for (const group of groups) {
    const key = JSON.stringify(factsOf(group, marks, listed));
    const kept = by.get(key);
    by.set(key, kept === undefined ? group : { ...kept, count: kept.count + group.count });
  }
  return [...by.values()];
};

const sameFacts = (a: unknown, b: Record<string, unknown>): boolean => JSON.stringify(a) === JSON.stringify(b);

/** The scenario's one `sheet` as a list of the folder, counted by its column that pairs the most. */
const sheetList = (scenario: Scenario): ScenarioList => {
  const paired = Math.max(0, ...scenario.sheet.columns.map((column) => Math.max(column.groupKeys, column.fileNames)));
  return { file: scenario.sheet.file, rows: scenario.sheet.rows, paired, normalized: paired };
};

const LIST_COLUMNS = ['File', 'Title'];

const columnsOf = (list: ScenarioList): readonly string[] => list.columns ?? LIST_COLUMNS;

/** A list's lines from line 1, as the sheet view sends them. */
const linesOf = (scenario: Scenario, list: ScenarioList | undefined): readonly string[] =>
  list === undefined ? scenario.sheet.lines : (list.lines ?? [columnsOf(list).join(','), [list.file, ...columnsOf(list).slice(1).map(() => 'A')].join(',')]);

/** A list's `listCandidates` entry, its `inPlan` the asked plan's sheet that reads it. */
const candidateOf = (scenario: Scenario, list: ScenarioList, sheets: readonly Record<string, unknown>[]): Record<string, unknown> => {
  const best = scenario.lists === undefined ? scenario.sheet.columns.findIndex((column) => Math.max(column.groupKeys, column.fileNames) === list.paired) : 0;
  const column = scenario.sheet.columns[best];
  const header = scenario.lists === undefined ? (column?.header ?? '') : 'File';
  const on = scenario.lists === undefined && column !== undefined && column.groupKeys > column.fileNames ? 'identifier' : 'fileName';
  return {
    source: 's1',
    file: list.file,
    inPlan: sheets.find((sheet) => sheet['source'] === 's1' && sheet['file'] === list.file)?.['id'] ?? null,
    read: list.error === undefined ? 'ok' : { error: list.error },
    rows: list.error === undefined ? list.rows : 0,
    columns: scenario.lists === undefined ? scenario.sheet.columns.length : columnsOf(list).length,
    best: list.error !== undefined || list.normalized === 0 ? null : { index: best, header, on, paired: list.paired, pairedNormalized: list.normalized, empty: 0 },
    grain: list.grain ?? null,
    cells: list.error === undefined ? (list.cells ?? []) : [],
    overlaps: (list.overlaps ?? []).map((overlap) => ({ source: 's1', ...overlap })),
  };
};

/** What the last look found in a staging source of the plan. */
export interface StagingState {
  readonly files: number;
  readonly sheetFiles: readonly string[];
}

/**
 * `planEdits` replace members of the stored draft's plan, as an earlier save
 * in Advanced would; `staging` is the last look's state of a plan source
 * named `up`.
 */
export const simpleServer = (scenario: Scenario, planEdits: Record<string, unknown> = {}, staging: StagingState | null = null): SimpleServer => {
  const base = recordTemplate();
  const saved: Record<string, unknown>[] = [];
  const runs: Record<string, unknown>[] = [];
  let revision = 0;
  let status = 'draft';
  let plan: Record<string, unknown> = { ...asRecord(fixture('import-draft.json')['plan'], []), ...planEdits };
  const detail = (): Record<string, unknown> => ({ ...fixture('import-draft.json'), revision, status, plan });
  const sources = (): unknown[] => [
    { source: 's1', scanned: '2026-10-21T09:00:00+00:00', files: scenario.files, sheetFiles: (scenario.lists ?? [scenario.sheet]).map((list) => list.file), error: null },
    ...(staging === null ? [] : [{ source: 'up', scanned: '2026-10-21T09:00:00+00:00', files: staging.files, sheetFiles: staging.sheetFiles, error: null }]),
  ];
  const record = (id: number, group: ScenarioGroup): Record<string, unknown> => {
    const key = `${scenario.name}_${String(id)}`;
    return {
      ...base,
      id,
      files: Array.from({ length: Math.min(3, BUCKET_FILES[group.files] ?? 1) }, (_, at) => ({ source: 's1', path: `${group.folder}/${key}_${String(at)}.jpg` })),
      rows: group.row ? [{ sheet: 'folder-list-1', key: `${key}#1` }] : [],
      media: [],
      flags: [],
      fate: 'create',
      resolved: { ...asRecord(base['resolved'], []), itemKey: key },
      slots: { 'dcterms:title': [{ type: 'literal', text: `Item ${String(id)}`, lang: null, label: null, isPublic: true, link: null }] },
    };
  };

  const sheetsOf = (asked: Record<string, unknown>): Record<string, unknown>[] => {
    const sheets = Array.isArray(asked['sheets']) ? asked['sheets'].filter(isRecord) : [];
    return sheets.map((sheet) => {
      const read = isRecord(sheet['read']) ? sheet['read'] : {};
      const line = typeof read['headerLine'] === 'number' ? read['headerLine'] : 1;
      const list = scenario.lists?.find((one) => one.file === sheet['file']);
      const lines = linesOf(scenario, list);
      const header = list?.columns !== undefined && line === (list.headerLine ?? 1) ? list.columns : (lines[line - 1] ?? '').split(',');
      const listed = fixture('preview-sheets.json')['sheets'];
      const fixtureSheet = asRecord(Array.isArray(listed) ? listed[0] : null, []);
      const fixtureRead = asRecord(fixtureSheet['read'], []);
      return {
        ...fixtureSheet,
        id: sheet['id'],
        source: sheet['source'],
        file: sheet['file'],
        lines,
        guess: { ...asRecord(fixtureSheet['guess'], []), headerLine: list === undefined ? scenario.sheet.headerLine : (list.headerLine ?? 1) },
        read: { ...fixtureRead, headerLine: line, rows: list?.rows ?? scenario.sheet.rows, notes: [], columns: header.map((name, index) => ({ header: name, index, mappable: true })) },
      };
    });
  };

  const preview: Handler = (_url, init) => {
    const body = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
    const asked = isRecord(body['plan']) ? body['plan'] : plan;
    const marks = marksOf(asked);
    const listed = (Array.isArray(asked['sheets']) ? asked['sheets'] : []).length > 0;
    const groups = merged(shownGroups(scenario, marks), marks, listed);
    const include = Array.isArray(body['include']) ? body['include'].map(String) : [];
    const answer: Record<string, unknown> = { sources: sources(), pattern: { groups: ['identifier', 'part'] } };
    const items = groups.reduce((sum, group) => sum + group.count, 0);
    const slots = isRecord(asked['slots']) && isRecord(asked['slots']['item']) ? asked['slots']['item'] : {};
    const typed = isRecord(slots['template']) && Array.isArray(slots['template']['sources']) && slots['template']['sources'].length > 0;
    for (const section of include) {
      if (section === 'shapes') {
        const groupsPage = pageOf(body['shapes'], 20);
        const rows = groups.map((group, at) => ({
          facts: factsOf(group, marks, listed),
          count: group.count,
          odd: group.count < items / 50 || group.files === '7+',
          samples: [record(at * 3 + 1, group), record(at * 3 + 2, group), record(at * 3 + 3, group)],
        }));
        answer['shapes'] = { records: items, groups: paged(rows, groupsPage) };
      } else if (section === 'histogram') {
        answer['histogram'] = ['0', '1', '2', '3', '4-6', '7+'].map((files) => ({
          files,
          count: files === '0' ? scenario.unpairedRows : groups.filter((group) => group.files === files).reduce((sum, group) => sum + group.count, 0),
        }));
      } else if (section === 'anomalies') {
        answer['anomalies'] = groups.some((group) => group.files === '7+') ? [{ largeGroup: { record: 1, files: 200, median: 1 } }] : [];
      } else if (section === 'flags') {
        const policies = isRecord(asked['flags']) ? asked['flags'] : {};
        answer['flags'] = scenario.flags
          .filter((flag) => listed || flag.kind === 'not-media')
          .map((flag) => ({
            kind: flag.kind,
            slot: flag.slot,
            policy: policies[flag.kind] ?? (['not-media', 'cells-count', 'exists'].includes(flag.kind) ? 'leave' : 'keep'),
            count: flag.count,
            samples: [{ subject: { record: 7 }, record: 7, detail: flag.detail }],
          }));
      } else if (section === 'identical') {
        const set = { source: 's1', size: 20, sha256: 'a'.repeat(64), paths: ['photos/a.jpg', 'photos/a (2).jpg'] };
        answer['identical'] = { files: scenario.identical, sets: paged(scenario.identical > 0 ? [set] : [], pageOf(body['identical'])) };
      } else if (section === 'unpaired') {
        const query = isRecord(body['unpaired']) ? body['unpaired'] : {};
        const rows = listed ? Array.from({ length: Math.min(scenario.unpairedRows, 3) }, (_, at) => ({ row: { sheet: 'folder-list-1', key: `lost_${String(at)}#1` }, record: 900 + at, title: null })) : [];
        const files = listed ? Array.from({ length: Math.min(scenario.unpairedFiles, 3) }, (_, at) => ({ file: { source: 's1', path: `photos/IMG_${String(at)}.jpg` }, record: 950 + at })) : [];
        const rowsPage = pageOf(query['rows']);
        const filesPage = pageOf(query['files']);
        answer['unpaired'] = {
          rows: { ...paged(rows, rowsPage), total: listed ? scenario.unpairedRows : 0 },
          files: { ...paged(files, filesPage), total: listed ? scenario.unpairedFiles : 0 },
        };
      } else if (section === 'entries') {
        const query = isRecord(body['entries']) ? body['entries'] : {};
        const text = typeof query['text'] === 'string' ? query['text'].toLowerCase() : null;
        const unmatched = Array.from({ length: Math.min(scenario.unmatchedFiles ?? 0, 3) }, (_, at) => `stray/odd name ${String(at)}.jpg`);
        const files = query['reason'] === 'unmatched' ? unmatched : scenario.examples.filter((path) => text === null || path.toLowerCase().includes(text));
        const shown = paged(
          files.map((path, at) => ({ entry: { file: { source: 's1', path } }, record: at + 1, left: null, unmatched: false, example: true, flags: [] })),
          pageOf(query),
        );
        answer['entries'] = query['reason'] === 'unmatched' ? { ...shown, total: scenario.unmatchedFiles ?? 0 } : shown;
      } else if (section === 'nameWords') {
        const query = isRecord(body['nameWords']) ? body['nameWords'] : {};
        const path = typeof query['path'] === 'string' ? query['path'] : '';
        const name = path.slice(path.lastIndexOf('/') + 1);
        const parts = name.split(/[_\-. ]/u);
        const known = scenario.words[path] ?? {};
        answer['nameWords'] = {
          parts,
          names: scenario.files,
          places: parts.map((part, at) => {
            const words = (known[at] ?? [[part, scenario.files]]).map(([word, count]) => ({ text: word, count }));
            return paged(words, pageOf(query, 12));
          }),
        };
      } else if (section === 'sheets') {
        answer['sheets'] = sheetsOf(asked);
      } else if (section === 'columnMatches') {
        answer['columnMatches'] = sheetsOf(asked).map((sheet) => {
          const read = asRecord(sheet['read'], []);
          const list = scenario.lists?.find((one) => one.file === sheet['file']);
          if (list !== undefined) {
            return {
              sheet: sheet['id'],
              rows: list.rows,
              columns: columnsOf(list).map((header, index) => ({ index, header, groupKeys: 0, fileNames: index === 0 ? list.paired : 0, empty: 0 })),
            };
          }
          const right = read['headerLine'] === scenario.sheet.headerLine;
          return {
            sheet: sheet['id'],
            rows: scenario.sheet.rows,
            columns: right
              ? scenario.sheet.columns.map((column, index) => ({ index, ...column }))
              : [{ index: 0, header: scenario.sheet.lines[0] ?? '', groupKeys: 0, fileNames: 0, empty: 0 }],
          };
        });
      } else if (section === 'listCandidates') {
        const sheets = Array.isArray(asked['sheets']) ? asked['sheets'].filter(isRecord) : [];
        answer['listCandidates'] = (scenario.lists ?? [sheetList(scenario)])
          .map((list) => candidateOf(scenario, list, sheets))
          .sort((a, b) => (isRecord(b['best']) ? Number(b['best']['paired']) : -1) - (isRecord(a['best']) ? Number(a['best']['paired']) : -1) || String(a['file']).localeCompare(String(b['file'])));
      } else if (section === 'values') {
        const query = pageOf(body['values']);
        const types = typed ? scenario.typeValues.map(([text, count]) => ({ text, count, spellings: [text] })) : [];
        answer['values'] = [
          { slot: 'sets', values: paged([], query) },
          { slot: 'template', values: paged(types, query) },
          { slot: 'class', values: paged([], query) },
        ];
      } else if (section === 'records') {
        const query = isRecord(body['records']) ? body['records'] : {};
        const hits = groups.flatMap((group, at) =>
          query['shape'] === undefined || sameFacts(query['shape'], factsOf(group, marks, listed))
            ? Array.from({ length: group.count }, (_, n) => ({ at: at * 100_000 + n + 1, group }))
            : [],
        );
        const page = pageOf(query);
        const slice = hits.slice((page.page - 1) * page.perPage, page.page * page.perPage).map((hit) => record(hit.at, hit.group));
        answer['records'] = { total: hits.length, page: page.page, perPage: page.perPage, items: slice };
      } else if (section === 'check') {
        const check = asRecord(fixture('preview-check.json')['check'], []);
        answer['check'] = {
          ...check,
          counts: [
            { kind: 'item', does: 'create', count: items },
            { kind: 'media', does: 'create', count: scenario.files },
          ],
          newSets: [],
          newTypes: [],
          newFields: [],
          withoutTemplate: 0,
          digest: `digest-${String(revision)}`,
        };
      }
    }
    return jsonResponse(answer);
  };

  const field = (term: string, label: string, id: number): Record<string, unknown> => {
    const page = loadFixture('properties-page.json');
    const first: unknown = Array.isArray(page) ? page[0] : null;
    const local = term.slice(term.indexOf(':') + 1);
    return { ...asRecord(first, []), 'o:id': id, 'o:term': term, 'o:label': label, 'o:local_name': local };
  };
  const fields = [
    field('dcterms:title', 'Title', 1),
    field('dcterms:date', 'Date', 7),
    field('dcterms:type', 'Type', 8),
    field('dcterms:description', 'Description', 4),
    field('dcterms:identifier', 'Identifier', 10),
  ];

  const handler = route({
    '/api/romeka/archive': () =>
      jsonResponse({ roots: ['archive'], root: 'archive', path: '', entries: [{ kind: 'file', name: scenario.sheet.file, path: scenario.sheet.file, size: 100 }] }),
    '/api/romeka/imports': (_url, init) => (init?.method === 'POST' ? jsonResponse(detail()) : jsonResponse(fixture('history.json'))),
    [PATH]: (_url, init) => {
      if (init?.method === 'PUT') {
        const sent = asRecord(JSON.parse(typeof init.body === 'string' ? init.body : '{}'), []);
        plan = asRecord(sent['plan'], []);
        saved.push(plan);
        revision += 1;
      }
      return jsonResponse(detail());
    },
    [`${PATH}/preview`]: preview,
    [`${PATH}/run`]: (_url, init) => {
      runs.push(asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []));
      status = 'running';
      return jsonResponse({ jobId: 7 });
    },
    [`${PATH}/template.csv`]: () => new Response('﻿Identifier,Files,Title [dcterms:title]\r\nitem_1,a.jpg,A\r\n', { status: 200 }),
    '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
    '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
    '/api/properties': () => listResponse(fields, fields.length),
  });
  return { handler, saved, runs };
};
