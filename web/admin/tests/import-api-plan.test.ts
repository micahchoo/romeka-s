/**
 * The import plan (`src/api/import-plan.ts`, `import-plan-read.ts`) against the Rust side: the
 * three plans the server's own tests keep, and every plan the routes answer.
 * A plan the client parses must serialise back to the same JSON, because the
 * admin saves what it parsed.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { parseImportPlan } from '../src/api/import-plan-read';
import { ParseError } from '../src/api/parse';
import { loadFixture } from './loadFixture';

const HERE = dirname(fileURLToPath(import.meta.url));
const RUST_PLANS = resolve(HERE, '../../../crates/omeka-api/src/import2/fixtures');

const rustPlan = (name: string): unknown => {
  const value: unknown = JSON.parse(readFileSync(resolve(RUST_PLANS, name), 'utf8'));
  return value;
};

const roundTrip = (raw: unknown): unknown => {
  const text: unknown = JSON.stringify(parseImportPlan(raw));
  if (typeof text !== 'string') {
    throw new Error('a plan serialises to text');
  }
  const back: unknown = JSON.parse(text);
  return back;
};

const planOf = (fixture: string, member = 'plan'): unknown => {
  const view = loadFixture(`import/${fixture}`);
  if (typeof view !== 'object' || view === null || !(member in view)) {
    throw new Error(`${fixture} has no ${member}`);
  }
  return Reflect.get(view, member);
};

describe('the plan round trip', () => {
  it.each(['default.json', 'full.json', 'delete.json'])('rust fixture %s serialises back to itself', (name) => {
    const raw = rustPlan(name);
    expect(roundTrip(raw)).toEqual(raw);
  });

  it.each([
    ['import-draft.json', 'plan'],
    ['import-saved.json', 'plan'],
    ['import-running.json', 'plan'],
    ['import-running.json', 'ranPlan'],
    ['import-done.json', 'ranPlan'],
    ['import-copied.json', 'plan'],
    ['import-saved-anomalies.json', 'plan'],
    ['import-sheet-added.json', 'plan'],
  ])('the plan in %s (%s) serialises back to itself', (fixture, member) => {
    const raw = planOf(fixture, member);
    expect(roundTrip(raw)).toEqual(raw);
  });

  it('a plan with every decision kind keeps each one', () => {
    const plan = parseImportPlan(planOf('import-saved.json'));
    expect(new Set(plan.decisions.map((decision) => decision.kind))).toEqual(
      new Set(['move', 'leave', 'set', 'match', 'flag', 'pair', 'many']),
    );
  });
});

describe('what the plan boundary refuses', () => {
  const base = (): Record<string, unknown> => {
    const plan = planOf('import-draft.json');
    if (typeof plan !== 'object' || plan === null) {
      throw new Error('a plan is an object');
    }
    return { ...plan };
  };

  it('refuses a member it does not know, since a save would lose it', () => {
    expect(() => parseImportPlan({ ...base(), surprise: true })).toThrow(ParseError);
  });

  it('refuses a version other than 2 and a v1 plan', () => {
    expect(() => parseImportPlan({ ...base(), version: 1 })).toThrow(ParseError);
    expect(() => parseImportPlan({ setLevel: 'level1', pattern: null })).toThrow(ParseError);
  });

  it('refuses a flag kind, a delimiter and an owner the server would not take', () => {
    expect(() => parseImportPlan({ ...base(), flags: { 'no-such-flag': 'keep' } })).toThrow(ParseError);
    const plan = parseImportPlan(planOf('import-saved.json'));
    const sheet = plan.sheets[0];
    expect(sheet?.read.delimiter).toBe('auto');
    expect(() =>
      parseImportPlan({
        ...base(),
        defaults: { visibility: 'private', owner: 'someone', sites: [], template: null },
      }),
    ).toThrow(ParseError);
  });

  it('keeps the sheet a plan names, leaves an absent one absent and reads null as absent', () => {
    const plan = parseImportPlan(rustPlan('full.json'));
    const [first, ...others] = plan.sheets;
    if (first === undefined) {
      throw new Error('the Rust plan has a sheet');
    }
    const withName = (sheetName: unknown): unknown => ({
      ...plan,
      sheets: [{ ...first, read: { ...first.read, sheetName } }, ...others],
    });
    const named = withName('Register');
    expect(roundTrip(named)).toEqual(JSON.parse(JSON.stringify(named)));
    expect(parseImportPlan(named).sheets[0]?.read.sheetName).toBe('Register');
    expect(first.read).not.toHaveProperty('sheetName');
    expect(parseImportPlan(withName(null)).sheets[0]?.read).not.toHaveProperty('sheetName');
    expect(() => parseImportPlan(withName(2))).toThrow(ParseError);
  });

  it('keeps an absent value map absent and a given one as it was', () => {
    const plan = parseImportPlan(planOf('import-saved.json'));
    expect(plan.slots.item['sets']?.map).toEqual({ Scans: '' });
    expect(plan.slots.item['template']).not.toHaveProperty('map');
  });

  it('reads the anomaly answers the server saved, and only when it has them', () => {
    const saved = parseImportPlan(planOf('import-saved-anomalies.json'));
    expect(saved.anomalies).toEqual({ identical: 'keepAll', unmatchedFiles: 'own', itemsWithoutRow: 'leave' });
    expect(parseImportPlan(planOf('import-saved.json')).anomalies).toBeUndefined();
    const answers = { identical: 'keepOne', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' };
    const withAnswers = parseImportPlan({ ...base(), anomalies: answers });
    expect(withAnswers.anomalies).toEqual(answers);
    expect(parseImportPlan(base()).anomalies).toBeUndefined();
    expect(() => parseImportPlan({ ...base(), anomalies: { identical: 'keepSome', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' } })).toThrow(ParseError);
  });
});
