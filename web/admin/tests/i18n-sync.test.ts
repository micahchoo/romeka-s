/**
 * The namespace tables against the shell catalogue and each other.
 *
 * A key id is unique across the shell and every namespace, so one id names one
 * string. While the split moves keys out of the shell catalogue, a key may sit
 * in both for a phase; then its text must agree in every locale, so the move
 * can never change what a reader sees.
 */
import { describe, expect, it } from 'vitest';
import { CATALOGUE_LOADERS, LOCALES } from '../src/i18n';

const ENGLISH = import.meta.glob<Readonly<Record<string, string>>>(
  '../src/i18n/namespaces/*/index.ts',
  { eager: true, import: 'en' },
);
const LOCALE_FILES = import.meta.glob<Readonly<Record<string, unknown>>>(
  '../src/i18n/namespaces/*/locales/*.ts',
  { eager: true },
);

const idOf = (path: string): string => path.split('/').at(-2) ?? path;
const namespaceOf = (path: string): string => path.split('/').at(-3) ?? path;

/** A namespace's table in one locale, read from its locale file or its English. */
const tableOf = (id: string, locale: string): Readonly<Record<string, unknown>> | undefined => {
  if (locale === 'en') {
    return Object.entries(ENGLISH).find(([path]) => idOf(path) === id)?.[1];
  }
  const module = Object.entries(LOCALE_FILES).find(
    ([path]) => namespaceOf(path) === id && path.endsWith(`/${locale}.ts`),
  )?.[1];
  const table = module?.[locale];
  return typeof table === 'object' && table !== null ? Object.fromEntries(Object.entries(table)) : undefined;
};

const NAMESPACES = Object.keys(ENGLISH).map(idOf);

describe('namespace tables', () => {
  it('a_key_id_is_in_at_most_one_namespace', () => {
    const owner = new Map<string, string>();
    const twice: string[] = [];
    for (const id of NAMESPACES) {
      for (const key of Object.keys(tableOf(id, 'en') ?? {})) {
        const other = owner.get(key);
        if (other !== undefined) {
          twice.push(`${key}: ${other}, ${id}`);
        }
        owner.set(key, id);
      }
    }
    expect(twice).toEqual([]);
  });

  it('every_namespace_has_a_complete_table_in_every_locale', () => {
    for (const id of NAMESPACES) {
      const english = Object.keys(tableOf(id, 'en') ?? {});
      for (const locale of LOCALES) {
        const table = tableOf(id, locale);
        expect(table, `${id} ${locale}`).toBeDefined();
        expect(english.filter((key) => typeof table?.[key] !== 'string'), `${id} ${locale}`).toEqual([]);
      }
    }
  });

  it('a_key_still_in_the_shell_catalogue_reads_the_same_in_its_namespace', async () => {
    const differ: string[] = [];
    for (const locale of LOCALES) {
      const shell: Readonly<Record<string, string>> = await CATALOGUE_LOADERS[locale]();
      for (const id of NAMESPACES) {
        for (const [key, text] of Object.entries(tableOf(id, locale) ?? {})) {
          if (Object.hasOwn(shell, key) && shell[key] !== text) {
            differ.push(`${locale} ${key}`);
          }
        }
      }
    }
    expect(differ).toEqual([]);
  });
});
