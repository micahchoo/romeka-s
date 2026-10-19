/**
 * The generated block registry explains every field. Each field of every block,
 * Romeka block and styling schema names a label and a help line that every
 * locale's page editor table holds, and each `when` names a real sibling of the same list,
 * so a form can never show a bare label or wait on a key nobody writes.
 */
import { describe, expect, it } from 'vitest';
import raw from '../src/generated/blocks.json';
import { parseBlockRegistry } from '../src/pages/sites/blockSchema';
import type { BlockField } from '../src/pages/sites/blockSchema';
import { en } from '../src/pages/sites/editor/messages';
import type { EditorMessages } from '../src/pages/sites/editor/messages';
import { es } from '../src/pages/sites/editor/locales/es';
import { hi } from '../src/pages/sites/editor/locales/hi';
import { kn } from '../src/pages/sites/editor/locales/kn';
import { ml } from '../src/pages/sites/editor/locales/ml';
import { mr } from '../src/pages/sites/editor/locales/mr';
import { ne } from '../src/pages/sites/editor/locales/ne';
import { ta } from '../src/pages/sites/editor/locales/ta';
import { te } from '../src/pages/sites/editor/locales/te';

const TABLES: Readonly<Record<string, EditorMessages>> = { en, es, hi, kn, ml, mr, ne, ta, te };

const registry = parseBlockRegistry(raw);

/** Every field list a form renders, named for the failure message. */
const lists = (): readonly (readonly [string, readonly BlockField[]])[] => {
  const top: (readonly [string, readonly BlockField[]])[] = [
    ...registry.layouts.map((layout) => [layout.name, layout.fields] as const),
    ...registry.romeka.map((block) => [block.name, block.fields] as const),
    ['blockLayoutData', registry.blockLayoutData.fields],
    ['pageLayoutData', registry.pageLayoutData.fields],
  ];
  return top.flatMap(([name, fields]) => [
    [name, fields] as const,
    ...fields.flatMap((field) =>
      field.type === 'repeat' ? [[`${name}.${field.key}`, field.fields] as const] : [],
    ),
  ]);
};

describe('the block registry', () => {
  it('gives every field a label and a help line in every language', () => {
    for (const [locale, table] of Object.entries(TABLES)) {
      const t = (key: keyof EditorMessages): string => table[key];
      for (const [name, fields] of lists()) {
        for (const field of fields) {
          const where = `${locale} ${name}.${field.key}`;
          expect(t(field.label).trim(), `${where} label`).not.toBe('');
          expect(t(field.help).trim(), `${where} help`).not.toBe('');
          expect(t(field.help), `${where} help repeats the label`).not.toBe(t(field.label));
        }
      }
    }
  });

  it('names a real sibling in every condition', () => {
    const conditions = lists().flatMap(([name, fields]) =>
      fields.flatMap((field) => (field.when === null ? [] : [{ name, field, when: field.when, fields }])),
    );
    expect(conditions.length).toBeGreaterThan(0);
    for (const { name, field, when, fields } of conditions) {
      const sibling = fields.find((entry) => entry.key === when.field);
      expect(sibling, `${name}.${field.key} waits on ${when.field}`).toBeDefined();
      expect(sibling?.key).not.toBe(field.key);
      if (when.test === 'equals' && sibling?.type === 'choice') {
        expect(sibling.options.map((option) => option.value)).toContain(when.value);
      }
    }
  });

  it('rejects a field without a help key', () => {
    const broken = structuredClone(raw);
    const first = broken.layouts.find((layout) => layout.fields.length > 0)?.fields[0];
    if (first === undefined) {
      throw new Error('the registry has no field');
    }
    Reflect.deleteProperty(first, 'help');
    expect(() => parseBlockRegistry(broken)).toThrow(/help/);
  });
});
