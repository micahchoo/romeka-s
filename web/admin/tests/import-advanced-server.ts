/**
 * A stand-in for the import routes the Advanced screens call, over a draft of
 * `items` items with two files each (`Letters/item_00001_front.jpg` and
 * `…_back.jpg`) and one more file, `Letters/notes.txt`, and one list
 * (`register.csv`) joined on its Identifier column.
 *
 * The pattern is matched here as the server matches it: a regex over each
 * file's path, the `identifier` group naming the item. A preview may carry a
 * plan in place of the stored one, as the pattern editor sends, and is
 * answered from that plan. Every other section is the P4-api fixture's, so
 * the screens run through the real client and its parsers.
 */
import { asRecord, isRecord } from '../src/api/parse';
import { patternOf } from '../src/pages/import/example';
import { loadFixture } from './loadFixture';
import { flatPlan, withMembers } from './import-plan-members';
import { jsonResponse, listResponse, route } from './stub';
import type { Handler } from './stub';

const fixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const sectionOf = (name: string, section: string): unknown => fixture(name)[section];

export const PATH = '/api/romeka/imports/1';
const pad = (n: number): string => String(n).padStart(5, '0');

export const MARKS = [{ kind: 'words', words: ['front', 'back'], sep: '_' }] as const;

const column = (index: number, header: string) => ({ index, header });

/** The stored plan: a pattern written by one mark, a list joined on Identifier, two field rules and four decisions. */
export const startPlan = (): Record<string, unknown> => withMembers(asRecord(fixture('import-draft.json')['plan'], []), {
  grouping: { pattern: patternOf(MARKS), caseFold: false },
  example: { path: 'Letters/item_00001_front.jpg', marks: MARKS },
  sheets: [
    {
      id: 'register',
      source: 's1',
      file: 'register.csv',
      hash: null,
      read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
      rowKind: 'item',
      join: { on: 'identifier', column: column(0, 'Identifier'), normalize: { caseFold: false, leadingZeros: false, separators: false } },
      itemKey: null,
      columns: [],
    },
  ],
  slots: {
    item: {
      'dcterms:title': { sources: [{ from: 'column', sheet: 'register', column: column(1, 'Title') }, { from: 'group', name: 'identifier' }], combine: 'first' },
      sets: { sources: [{ from: 'folder', level: 1 }], combine: 'first' },
    },
    media: {},
    itemSet: {},
  },
  decisions: [
    { kind: 'move', file: { file: { source: 's1', path: 'Letters/item_00002_back.jpg' } }, to: 'own' },
    { kind: 'leave', entry: { file: { source: 's1', path: 'Letters/notes.txt' } } },
    { kind: 'pair', row: { sheet: 'register', key: 'item_00009#1' }, file: { source: 's1', path: 'Letters/item_00003_front.jpg' } },
    {
      kind: 'many',
      entries: [1, 4, 5, 6, 7].map((n) => ({ file: { source: 's1', path: `Letters/item_${pad(n)}_front.jpg` } })),
      change: { kind: 'leave' },
    },
  ],
});

export interface AdvancedServer {
  readonly handler: Handler;
  /** The plan of each PUT, in order, with its one series' members flat (`flatPlan`). */
  readonly saved: Record<string, unknown>[];
  /** The plan each preview read, stored or sent, flat as `saved`. */
  readonly previewed: Record<string, unknown>[];
}

const paged = (rows: readonly unknown[], query: unknown): Record<string, unknown> => {
  const ask = isRecord(query) ? query : {};
  const perPage = typeof ask['perPage'] === 'number' ? ask['perPage'] : 50;
  const page = typeof ask['page'] === 'number' ? ask['page'] : 1;
  return { total: rows.length, page, perPage, items: rows.slice((page - 1) * perPage, page * perPage) };
};

const NAMED = /\(\?<([A-Za-z_][A-Za-z0-9_]*)>/g;

const patternState = (pattern: unknown): unknown => {
  if (typeof pattern !== 'string') {
    return 'missing';
  }
  try {
    new RegExp(pattern, 'u');
  } catch (error) {
    return { syntax: error instanceof Error ? error.message : String(error) };
  }
  const groups = [...pattern.matchAll(NAMED)].map((found) => found[1] ?? '');
  return groups.includes('identifier') ? { groups } : 'noIdentifier';
};

const bucketOf = (files: number): string => (files >= 7 ? '7+' : files >= 4 ? '4-6' : String(files));

export const advancedServer = (items: number): AdvancedServer => {
  const paths = [...Array.from({ length: items }, (_, at) => [`Letters/item_${pad(at + 1)}_front.jpg`, `Letters/item_${pad(at + 1)}_back.jpg`]).flat(), 'Letters/notes.txt'];
  const saved: Record<string, unknown>[] = [];
  const previewed: Record<string, unknown>[] = [];
  let plan = startPlan();
  let revision = 0;
  const draft = (): Record<string, unknown> => ({ ...fixture('import-draft.json'), plan, revision });

  const grouped = (asked: Record<string, unknown>) => {
    const grouping = asRecord(asked['grouping'], []);
    const state = patternState(grouping['pattern']);
    const regex = isRecord(state) && 'groups' in state && typeof grouping['pattern'] === 'string' ? new RegExp(grouping['pattern'], 'u') : null;
    const groups = new Map<string, string[]>();
    const unfit: string[] = [];
    for (const path of paths) {
      const key = regex?.exec(path)?.groups?.['identifier'];
      if (key === undefined) {
        unfit.push(path);
      } else {
        groups.set(key, [...(groups.get(key) ?? []), path]);
      }
    }
    return { state, groups, unfit };
  };

  const preview: Handler = (_url, init) => {
    const body = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
    const include = Array.isArray(body['include']) ? body['include'].map(String) : [];
    const asked = flatPlan(isRecord(body['plan']) ? body['plan'] : plan);
    previewed.push(asked);
    const { state, groups, unfit } = grouped(asked);
    const answer: Record<string, unknown> = { sources: sectionOf('preview-records.json', 'sources'), pattern: state };
    const fileEntry = (path: string) => ({ entry: { file: { source: 's1', path } }, record: null, left: null, unmatched: unfit.includes(path), example: true, flags: [] });
    for (const section of include) {
      if (section === 'entries') {
        const query = isRecord(body['entries']) ? body['entries'] : {};
        const rows = query['reason'] === 'unmatched' ? unfit : paths;
        answer['entries'] = paged(rows.map(fileEntry), query);
      } else if (section === 'histogram') {
        const counts = new Map<string, number>();
        for (const files of groups.values()) {
          counts.set(bucketOf(files.length), (counts.get(bucketOf(files.length)) ?? 0) + 1);
        }
        answer['histogram'] = [...counts].map(([files, count]) => ({ files, count }));
      } else if (section === 'sheets') {
        const columns = ['Identifier', 'Title', 'Date', 'Notes'].map((header, index) => ({ index, header, mappable: true }));
        const encoding = { encoding: 'utf-8', confidence: 1 };
        answer['sheets'] = [
          {
            id: 'register',
            source: 's1',
            file: 'register.csv',
            found: true,
            error: null,
            read: { hash: 'h', encoding, delimiter: ',', delimiterGuessed: true, headerLine: 1, rows: items, cellsCount: 0, blankLines: 0, cellsWithSpaces: 0, cellsSuspect: 0, columns, notes: [], workbook: null },
            guess: { encoding, unsupported: null, delimiter: ',', headerLine: 1 },
            lines: ['Identifier,Title,Date,Notes', 'item_00001,Letter 1,1890,'],
          },
        ];
      } else if (section === 'columnMatches') {
        answer['columnMatches'] = [
          {
            sheet: 'register',
            rows: items,
            columns: ['Identifier', 'Title', 'Date', 'Notes'].map((header, index) => ({ index, header, groupKeys: index === 0 ? items : 0, fileNames: 0, empty: index === 3 ? items : 0 })),
          },
        ];
      } else if (section === 'unpaired') {
        answer['unpaired'] = { rows: paged([], {}), files: paged([{ file: { source: 's1', path: paths[0] }, record: 1 }], {}), items: 1 };
      } else if (section === 'check') {
        const check = asRecord(sectionOf('preview-check.json', 'check'), []);
        const action = asRecord(asked['action'], [])['action'];
        answer['check'] = { ...check, action, deletes: action === 'delete' ? items : 0 };
      } else if (section === 'orphans') {
        answer['orphans'] = [{ decision: 1, reason: 'missingEntry', entry: { file: { source: 's1', path: 'Letters/notes.txt' } } }];
      } else {
        const name = {
          flags: 'preview-flags.json',
          identical: 'preview-identical.json',
          values: 'preview-values.json',
          shapes: 'preview-shapes.json',
          records: 'preview-records.json',
          listCandidates: 'preview-list-candidates.json',
        }[section];
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
        plan = asRecord(body['plan'], []);
        saved.push(flatPlan(plan));
        revision += 1;
      }
      return jsonResponse(draft());
    },
    [`${PATH}/preview`]: preview,
    '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
    '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
    '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
    '/api/sites': () => listResponse(loadFixture('sites-page.json'), 1),
    '/api/users': () => listResponse(loadFixture('users-page.json'), 1),
  });
  return { handler, saved, previewed };
};
