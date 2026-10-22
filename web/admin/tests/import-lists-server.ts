/**
 * A stand-in for the import routes the review lists call, over a draft of any
 * size: `items` item records, one file each (`Letters/item_00001.jpg`), every
 * seventh file left out by a decision, and some items a row with no file
 * (`hasFile`).
 * The answers are the shapes of the P4-api fixtures, with the synthetic rows
 * in place of the fixture's, so the pages run through the real client and its
 * parsers. It answers only what each request asks, as the server does.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { asRecord, isRecord } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import { flatPlan } from './import-plan-members';
import { planProposal } from './import-plan-proposal';
import { jsonResponse, listResponse, route } from './stub';
import type { Handler } from './stub';

const fixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const sectionOf = (name: string, section: string): unknown => fixture(name)[section];

export const PATH = '/api/romeka/imports/1';
export const pad = (n: number): string => String(n).padStart(5, '0');
/** Item 3 and every tenth item are a list row with no file. */
export const hasFile = (n: number): boolean => n !== 3 && n % 10 !== 0;
export const countOf = (size: number, test: (n: number) => boolean): number =>
  Array.from({ length: size }, (_, index) => index + 1).filter(test).length;

const template = (): Record<string, unknown> => {
  const records = asRecord(sectionOf('preview-records.json', 'records'), []);
  const items = records['items'];
  if (!Array.isArray(items) || !isRecord(items[0])) {
    throw new Error('preview-records.json has no record');
  }
  return items[0];
};

export interface ListsServer {
  readonly handler: Handler;
  /** The plan of each PUT, in order, with its one series' members flat (`flatPlan`). */
  readonly saved: Record<string, unknown>[];
}

interface Page {
  readonly page?: number;
  readonly perPage?: number;
  readonly text?: string;
}

const paged = (rows: readonly unknown[], query: Page | undefined): Record<string, unknown> => {
  const perPage = Math.min(500, query?.perPage ?? 50);
  const page = query?.page ?? 1;
  return { total: rows.length, page, perPage, items: rows.slice((page - 1) * perPage, page * perPage) };
};

const pageOf = (raw: unknown): Page => {
  if (!isRecord(raw)) {
    return {};
  }
  const { page, perPage, text } = raw;
  return {
    ...(typeof page === 'number' ? { page } : {}),
    ...(typeof perPage === 'number' ? { perPage } : {}),
    ...(typeof text === 'string' ? { text } : {}),
  };
};

export const listsServer = (items: number, options: { readonly strays?: number; readonly listRows?: number } = {}): ListsServer => {
  const base = template();
  const records = Array.from({ length: items }, (_, index) => {
    const n = index + 1;
    return {
      ...base,
      id: n,
      files: !hasFile(n) ? [] : [{ source: 's1', path: `Letters/item_${pad(n)}.jpg` }],
      rows: [{ sheet: 'register', key: `item_${pad(n)}#1` }],
      media: [],
      flags: [],
      fate: 'create',
      resolved: { ...asRecord(base['resolved'], []), itemKey: `item_${pad(n)}` },
      slots: { 'dcterms:title': [{ type: 'literal', text: `Letter ${String(n)}`, lang: null, label: null, isPublic: true, link: null }] },
    };
  });
  const entries = records.flatMap((record) => {
    const files = record.files;
    return files.map((file) => ({
      entry: { file },
      record: record.id,
      left: record.id % 7 === 0 ? { decision: 0 } : null,
      unmatched: false,
      example: true,
      flags: [],
    }));
  });
  const strayRows = Array.from({ length: options.strays ?? 0 }, (_, index) => ({
    entry: { row: { sheet: 'list-1', key: `nobody_${String(index + 1)}#1` } },
    record: items + index + 1,
    left: null,
    unmatched: false,
    example: false,
    flags: [],
  }));
  const unpairedRows = records
    .filter((record) => record.files.length === 0)
    .map((record) => ({ row: record.rows[0], record: record.id, title: `Letter ${String(record.id)}` }));
  const unpairedFiles = records
    .filter((record) => record.id % 10 === 1)
    .map((record) => ({ file: record.files[0], record: record.id }));

  const saved: Record<string, unknown>[] = [];
  let revision = 0;
  const draft = (): Record<string, unknown> => ({ ...fixture('import-draft.json'), revision });

  const preview: Handler = (_url, init) => {
    const body = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
    const include = Array.isArray(body['include']) ? body['include'].map(String) : [];
    const answer: Record<string, unknown> = { sources: sectionOf('preview-records.json', 'sources'), pattern: sectionOf('preview-records.json', 'pattern') };
    for (const section of include) {
      if (section === 'records') {
        const query = isRecord(body['records']) ? body['records'] : {};
        const text = typeof query['text'] === 'string' ? query['text'] : null;
        const files = typeof query['files'] === 'string' ? query['files'] : null;
        const hits = records.filter(
          (record) =>
            (text === null || record.resolved.itemKey.includes(text)) &&
            (files === null || String(record.files.length) === files),
        );
        answer['records'] = paged(hits, pageOf(query));
      } else if (section === 'entries') {
        const query = isRecord(body['entries']) ? body['entries'] : {};
        if (query['paired'] === false || query['sheet'] !== undefined) {
          answer['entries'] = paged(strayRows, pageOf(query));
        } else {
          const used = typeof query['used'] === 'boolean' ? query['used'] : null;
          const text = typeof query['text'] === 'string' ? query['text'] : null;
          const hits = entries.filter(
            (entry) => (used === null || (entry.left === null) === used) && (text === null || entry.entry.file.path.includes(text)),
          );
          answer['entries'] = paged(hits, pageOf(query));
        }
      } else if (section === 'unpaired') {
        const query = isRecord(body['unpaired']) ? body['unpaired'] : {};
        const rowsQuery = pageOf(query['rows']);
        const filesQuery = pageOf(query['files']);
        const rows = unpairedRows.filter((row) => rowsQuery.text === undefined || row.row?.key.includes(rowsQuery.text) === true);
        const files = unpairedFiles.filter((file) => filesQuery.text === undefined || file.file?.path.includes(filesQuery.text) === true);
        answer['unpaired'] = { rows: paged(rows, rowsQuery), files: paged(files, filesQuery), items: new Set(unpairedFiles.map((file) => file.record)).size };
      } else if (section === 'sheets') {
        const sheets = sectionOf('preview-sheets.json', 'sheets');
        const first = Array.isArray(sheets) && isRecord(sheets[0]) ? sheets[0] : {};
        const read = asRecord(first['read'], []);
        const rows = (options.listRows ?? items) + (options.strays ?? 0);
        const columns = [
          { header: 'Identifier', index: 0, mappable: true },
          { header: 'Files', index: 1, mappable: true },
          { header: 'Title [dcterms:title]', index: 2, mappable: true },
          { header: 'Date [dcterms:date]', index: 3, mappable: true },
          { header: 'Notes', index: 4, mappable: true },
        ];
        answer['sheets'] = [{ ...first, id: 'list-1', file: 'lists/1.csv', source: 'staging', read: { ...read, rows, columns, notes: [] } }];
      } else if (section === 'proposal') {
        answer['proposal'] = planProposal(isRecord(body['plan']) ? body['plan'] : asRecord(draft()['plan'], []), entries.length);
      } else {
        const name = { shapes: 'preview-shapes.json', flags: 'preview-flags.json', check: 'preview-check.json', histogram: 'preview-histogram.json', identical: 'preview-identical.json' }[section];
        if (name !== undefined) {
          answer[section] = sectionOf(name, section);
        }
      }
    }
    return jsonResponse(answer);
  };

  const handler = route({
    [PATH]: (_url, init) => {
      if (init?.method === 'PUT') {
        const body = asRecord(JSON.parse(typeof init.body === 'string' ? init.body : '{}'), []);
        const plan = asRecord(body['plan'], []);
        saved.push(flatPlan(plan));
        revision += 1;
        return jsonResponse({ ...draft(), plan });
      }
      return jsonResponse(draft());
    },
    [`${PATH}/preview`]: preview,
    [`${PATH}/template.csv`]: () => new Response(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), 'fixtures/import/template.csv')), { status: 200 }),
    [`${PATH}/pairs.csv`]: () => new Response(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), 'fixtures/import/pairs.csv')), { status: 200 }),
    [`${PATH}/pairs`]: () => {
      revision += 1;
      const added = fixture('pairs-added.json');
      const pair = { kind: 'pair', row: { sheet: 'register', key: 'item_00003#1' }, file: { source: 's1', path: 'Letters/item_00001.jpg' } };
      const plan = { ...asRecord(draft()['plan'], []), decisions: [pair] };
      return jsonResponse({ ...added, import: { ...draft(), plan } });
    },
    [`${PATH}/sheets`]: () => {
      revision += 1;
      return jsonResponse({ ...fixture('import-sheet-added.json'), id: 1, revision });
    },
    '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
    '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
    '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
  });
  return { handler, saved };
};
