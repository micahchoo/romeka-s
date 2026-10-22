/**
 * The series seam (`src/pages/import/series.ts`) over the server's own v2
 * plan (`fixtures/full.json`): a v2 plan reads as the one series the server
 * lifts it to, and an edit made through that series writes the v2 plan an
 * edit of its members would. And the seam is the only way in: no other
 * import page reads or writes a v2 series member, as the type checker sees
 * the code.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import { parseImportPlan } from '../src/api/import-plan-read';
import type { ImportPlan } from '../src/api/import-plan';
import { seriesOf, withSeries } from '../src/pages/import/series';

const HERE = dirname(fileURLToPath(import.meta.url));

const fullPlan = (): ImportPlan =>
  parseImportPlan(JSON.parse(readFileSync(resolve(HERE, '../../../crates/omeka-api/src/import/fixtures/full.json'), 'utf8')));

describe('the one series of a v2 plan', () => {
  it('is the series the server lifts the plan to: all, over every file, with no set', () => {
    const plan = fullPlan();
    expect(seriesOf(plan)).toEqual({
      kind: 'series',
      id: 'all',
      set: null,
      scope: 'all',
      grouping: plan.grouping,
      example: plan.example,
      lists: plan.sheets,
      slots: plan.slots,
    });
    expect(seriesOf(plan, 'all')).toEqual(seriesOf(plan));
    expect(() => seriesOf(plan, 'letters')).toThrow('letters');
  });

  it('an edit through the one series writes the same v2 plan', () => {
    const plan = fullPlan();
    const [sheet] = plan.sheets;
    if (sheet === undefined) {
      throw new Error('full.json has a sheet');
    }
    const copy = { ...sheet, id: 'copy' };
    const grouping = { pattern: '^(?<identifier>[^/]+)$', caseFold: false };
    const slots = { ...plan.slots, media: {} };
    const edited = withSeries(plan, (series) => ({ ...series, grouping, example: null, lists: [...series.lists, copy], slots }));
    const direct: ImportPlan = { ...plan, grouping, example: null, sheets: [...plan.sheets, copy], slots };
    expect(edited).toEqual(direct);
    expect(JSON.stringify(edited)).toBe(JSON.stringify(direct));
    expect(parseImportPlan(JSON.parse(JSON.stringify(edited)))).toEqual(direct);
  });

  it('a change that keeps the series leaves the plan itself', () => {
    const plan = fullPlan();
    expect(withSeries(plan, (series) => series)).toBe(plan);
  });
});

/** The members of a v2 plan that v3 moves into each series (`plan/v3.rs`). */
const SERIES_MEMBERS: ReadonlySet<string> = new Set(['grouping', 'sheets', 'slots', 'example']);

/**
 * Every place an import page outside `series.ts` names one of the v2 plan's
 * series members, as `file:line member`. A place names one when the type
 * checker resolves the name to the member's declaration in `ImportPlan`: a
 * property read, a property of an object literal typed as or spread from a
 * plan, a destructured name, or an indexed type such as `ImportPlan['slots']`.
 */
const seriesMemberReaders = (): readonly string[] => {
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
    if (ts.isInterfaceDeclaration(node) && node.name.text === 'ImportPlan') {
      for (const member of node.members) {
        if (member.name !== undefined && ts.isIdentifier(member.name) && SERIES_MEMBERS.has(member.name.text)) {
          declarations.add(member);
        }
      }
    }
  });
  expect(declarations.size).toBe(SERIES_MEMBERS.size);
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
      if ((ts.isIdentifier(node) || ts.isStringLiteral(node)) && SERIES_MEMBERS.has(node.text) && symbolsOf(node).some(isPlanMember)) {
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
  it('no import file reads a v2 series member', () => {
    expect(seriesMemberReaders()).toEqual([]);
  }, 60_000);
});
