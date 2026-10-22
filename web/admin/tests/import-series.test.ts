/**
 * The series seam (`src/pages/import/series.ts`) over the server's own v3
 * plans (`fixtures/v3`): a series read and written through it is the series
 * of the tree, a screen of one series refuses a plan of several, the readers
 * keyed by a list find it in any series, the item sets rule is a free set and
 * never a series' rule, and Simple's state is kept under the series' id. And
 * the seam is the only way in: no other import page reads the plan's tree or
 * Simple's state, as the type checker sees the code.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import { parseImportPlan } from '../src/api/import-plan-tree-read';
import type { ImportPlan, Series } from '../src/api/import-plan';
import {
  SeveralSeriesError,
  allLists,
  listOf,
  seriesOf,
  setLevelsOf,
  setsOf,
  simpleOf,
  slotsOf,
  withList,
  withSeries,
  withSimple,
  withSlots,
} from '../src/pages/import/series';

const HERE = dirname(fileURLToPath(import.meta.url));

const rustPlan = (name: string): ImportPlan =>
  parseImportPlan(JSON.parse(readFileSync(resolve(HERE, `../../../crates/omeka-api/src/import/fixtures/v3/${name}`), 'utf8')));

const only = (plan: ImportPlan): Series => {
  const [level] = plan.structure.levels;
  if (level?.kind !== 'series') {
    throw new Error('the plan is one series');
  }
  return level;
};

describe('the series of a plan', () => {
  it('is the series of its tree; a plan of one needs no level', () => {
    const plan = rustPlan('one_series.json');
    expect(seriesOf(plan)).toBe(only(plan));
    expect(seriesOf(plan, 'files')).toBe(only(plan));
    expect(() => seriesOf(plan, 'letters')).toThrow('letters');
  });

  it('an edit through the series writes the plan an edit of its tree would', () => {
    const plan = rustPlan('one_series.json');
    const grouping = { pattern: '^(?<identifier>[^/]+)$', caseFold: false };
    const edited = withSeries(plan, (series) => ({ ...series, grouping, example: null }));
    const direct: ImportPlan = { ...plan, structure: { ...plan.structure, levels: [{ ...only(plan), grouping, example: null }] } };
    expect(edited).toEqual(direct);
    expect(JSON.stringify(edited)).toBe(JSON.stringify(direct));
    expect(parseImportPlan(JSON.parse(JSON.stringify(edited)))).toEqual(direct);
  });

  it('a change that keeps the series leaves the plan itself', () => {
    const plan = rustPlan('one_series.json');
    expect(withSeries(plan, (series) => series)).toBe(plan);
  });

  it('a screen of one series refuses a plan of several; a level names one, under its set level too', () => {
    const plan = rustPlan('bhc006.json');
    expect(() => seriesOf(plan)).toThrow(SeveralSeriesError);
    expect(seriesOf(plan, 'pn').lists.map((list) => list.id)).toEqual(['pn']);
    const edited = withSeries(plan, (series) => ({ ...series, grouping: { ...series.grouping, caseFold: true } }), 'pn');
    expect(seriesOf(edited, 'pn').grouping.caseFold).toBe(true);
    expect(seriesOf(edited, 'af')).toBe(seriesOf(plan, 'af'));
    expect(setLevelsOf(edited).map((level) => level.id)).toEqual(['project']);
  });

  it('finds and edits a list in whichever series holds it', () => {
    const plan = rustPlan('bhc006.json');
    expect(allLists(plan).map((list) => list.id)).toEqual(['af', 'pn', 'd']);
    const edited = withList(plan, 'd', (list) => ({ ...list, read: { ...list.read, headerLine: 2 } }));
    expect(listOf(edited, 'd')?.read.headerLine).toBe(2);
    expect(seriesOf(edited, 'd').lists[0]?.read.headerLine).toBe(2);
    expect(seriesOf(edited, 'af')).toBe(seriesOf(plan, 'af'));
    expect(withList(plan, 'nowhere', (list) => list)).toBe(plan);
  });
});

describe('the item sets rule', () => {
  const RULE = { sources: [{ from: 'folder' as const, level: 1 as const }], combine: 'first' as const };

  it('is written to a free set that names sets by title, never to the series', () => {
    const plan = rustPlan('archive_as_found.json');
    const edited = withSlots(plan, { ...slotsOf(plan), item: { ...slotsOf(plan).item, sets: RULE } });
    expect(seriesOf(edited).slots.item).toEqual({});
    expect(setsOf(edited)).toEqual([{ each: { ...RULE, names: 'title' } }]);
    expect(slotsOf(edited).item.sets).toEqual(RULE);
  });

  it('keeps how a set read as v2 read it names its sets, and goes when the rule goes', () => {
    const plan = rustPlan('archive_as_found.json');
    const lifted: ImportPlan = { ...plan, structure: { ...plan.structure, freeSets: [{ title: 'Kept', description: null }, { each: { ...RULE, names: 'text' } }] } };
    const mapped = { ...RULE, map: { Scans: '' } };
    const edited = withSlots(lifted, { ...slotsOf(lifted), item: { sets: mapped } });
    expect(setsOf(edited)).toEqual([{ title: 'Kept', description: null }, { each: { ...mapped, names: 'text' } }]);
    const item = Object.fromEntries(Object.entries(slotsOf(edited).item).filter(([slot]) => slot !== 'sets'));
    expect(setsOf(withSlots(edited, { ...slotsOf(edited), item }))).toEqual([{ title: 'Kept', description: null }]);
  });
});

describe('Simple’s state', () => {
  it('is kept under the series’ id, beside the other series’ states', () => {
    const plan = { ...rustPlan('one_series.json'), simple: { other: { answered: ['x'] } } };
    expect(simpleOf(plan)).toBeNull();
    const kept = withSimple(plan, { answered: ['identical'] });
    expect(kept.simple).toEqual({ other: { answered: ['x'] }, files: { answered: ['identical'] } });
    expect(simpleOf(kept)).toEqual({ answered: ['identical'] });
  });
});

/**
 * The members that hold the plan's tree and Simple's state, by the interface
 * that declares them (`api/import-plan.ts`): a page that reads one of them
 * reaches a series, a free set or a series' Simple state around the seam.
 */
const TREE_MEMBERS: Readonly<Record<string, readonly string[]>> = {
  ImportPlan: ['structure', 'simple'],
  Structure: ['levels', 'freeSets', 'outside'],
  SetLevel: ['levels'],
};
const NAMES: ReadonlySet<string> = new Set(Object.values(TREE_MEMBERS).flat());

/**
 * Every place an import page outside `series.ts` names one of those members,
 * as `file:line member`. A place names one when the type checker resolves the
 * name to the member's declaration: a property read, a property of an object
 * literal typed as or spread from a plan, a destructured name, or an indexed
 * type such as `ImportPlan['structure']`.
 */
const treeMemberReaders = (): readonly string[] => {
  const root = resolve(HERE, '..');
  const config = ts.getParsedCommandLineOfConfigFile(resolve(root, 'tsconfig.json'), {}, { ...ts.sys, onUnRecoverableConfigFileDiagnostic: () => undefined });
  if (config === undefined) {
    throw new Error('tsconfig.json reads');
  }
  const pages = config.fileNames.filter((file) => file.includes('/src/pages/import/'));
  const program = ts.createProgram(pages, config.options);
  const checker = program.getTypeChecker();
  const planFile = program.getSourceFile(resolve(root, 'src/api/import-plan.ts'));
  const declarations = new Set<ts.Node>();
  planFile?.forEachChild((node) => {
    const members = ts.isInterfaceDeclaration(node) ? TREE_MEMBERS[node.name.text] : undefined;
    if (ts.isInterfaceDeclaration(node) && members !== undefined) {
      for (const member of node.members) {
        if (member.name !== undefined && ts.isIdentifier(member.name) && members.includes(member.name.text)) {
          declarations.add(member);
        }
      }
    }
  });
  expect(declarations.size).toBe(Object.values(TREE_MEMBERS).flat().length);
  const isPlanMember = (symbol: ts.Symbol | undefined): boolean => (symbol?.declarations ?? []).some((declaration) => declarations.has(declaration));
  const propertiesOf = (type: ts.Type | undefined, name: string): readonly (ts.Symbol | undefined)[] =>
    type === undefined ? [] : (type.isUnion() ? type.types : [type]).map((one) => one.getProperty(name));
  const symbolsOf = (node: ts.Identifier | ts.StringLiteral): readonly (ts.Symbol | undefined)[] => {
    const parent = node.parent;
    const symbols = [checker.getSymbolAtLocation(node)];
    if ((ts.isPropertyAssignment(parent) || ts.isShorthandPropertyAssignment(parent)) && parent.name === node && ts.isObjectLiteralExpression(parent.parent)) {
      const literal = parent.parent;
      symbols.push(...propertiesOf(checker.getContextualType(literal), node.text));
      for (const property of literal.properties) {
        if (ts.isSpreadAssignment(property)) {
          symbols.push(...propertiesOf(checker.getTypeAtLocation(property.expression), node.text));
        }
      }
    }
    if (ts.isBindingElement(parent) && (parent.propertyName ?? parent.name) === node) {
      symbols.push(...propertiesOf(checker.getTypeAtLocation(parent.parent), node.text));
    }
    if (ts.isLiteralTypeNode(parent)) {
      let around: ts.Node = parent.parent;
      while (!ts.isIndexedAccessTypeNode(around) && !ts.isTypeReferenceNode(around) && ts.isTypeNode(around.parent)) {
        around = around.parent;
      }
      const object = ts.isIndexedAccessTypeNode(around) ? around.objectType : ts.isTypeReferenceNode(around) ? around.typeArguments?.[0] : undefined;
      if (object !== undefined) {
        symbols.push(...propertiesOf(checker.getTypeFromTypeNode(object), node.text));
      }
    }
    return symbols;
  };
  const found: string[] = [];
  for (const file of pages.filter((one) => !one.endsWith('/pages/import/series.ts'))) {
    const source = program.getSourceFile(file);
    const visit = (node: ts.Node): void => {
      if ((ts.isIdentifier(node) || ts.isStringLiteral(node)) && NAMES.has(node.text) && symbolsOf(node).some(isPlanMember)) {
        const { line } = ts.getLineAndCharacterOfPosition(node.getSourceFile(), node.getStart());
        found.push(`${file.slice(root.length + 1)}:${String(line + 1)} ${node.text}`);
      }
      node.forEachChild(visit);
    };
    if (source !== undefined) {
      visit(source);
    }
  }
  return found;
};

describe('the series seam', () => {
  it('no import file reads the plan’s tree or Simple’s state but series.ts', () => {
    expect(treeMemberReaders()).toEqual([]);
  }, 60_000);
});
