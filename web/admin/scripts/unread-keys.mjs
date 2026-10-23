#!/usr/bin/env node
/**
 * The catalogue keys no code reads. A key is read when the code outside the
 * catalogues names it in a string literal (`t('a.b')`, a table of keys, a
 * JSON registry), or a template literal could make it (`a.${kind}.title`).
 * Every namespace's English table under `src/i18n/namespaces/` is the list of
 * keys; its eight other locales and `sources.json` follow English.
 *
 *   node scripts/unread-keys.mjs            print each unread key, by namespace
 *   node scripts/unread-keys.mjs --remove   also delete them from every table
 *
 * `tests/i18n-unread-keys.test.ts` runs it and wants none.
 */
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const SRC = join(ROOT, 'src');
const NAMESPACES = join(SRC, 'i18n', 'namespaces');

const walk = (dir) =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });

/** A table's keys in file order: each `'key': ` or `"key": ` that starts a line. */
const keysIn = (text) => [...text.matchAll(/^\s*(['"])([^'"]+)\1\s*:/gmu)].map((found) => found[2]);

/** The code that may read a key: every source file but the namespace tables themselves. */
const code = () =>
  walk(SRC)
    .filter((path) => /\.(ts|tsx|json)$/u.test(path) && !path.startsWith(NAMESPACES + '/'))
    .map((path) => readFileSync(path, 'utf8'))
    .join('\n');

const escape = (text) => text.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');

/** What reads keys: each quoted literal, and each template literal with a hole as a pattern. */
const readers = (text) => {
  // Each quote with the run of key letters up to the same quote: overlapping, so a quote that closes one literal also opens the next try.
  const literals = new Set(["'", '"', '`'].flatMap((quote) => [...text.matchAll(new RegExp(`${quote}(?=([A-Za-z0-9_.: -]+)${quote})`, 'gu'))].map((found) => found[1])));
  const patterns = [...text.matchAll(/`([A-Za-z0-9_.:-]*(?:\$\{[^`}]*\}[A-Za-z0-9_.:-]*)+)`/gu)]
    .map((found) => found[1])
    .filter((template) => /^[A-Za-z0-9_-]+\./u.test(template))
    .map((template) => new RegExp(`^${template.split(/\$\{[^}]*\}/u).map(escape).join('[A-Za-z0-9_.-]+')}$`, 'u'));
  return { literals, patterns };
};

export const unreadKeys = () => {
  const { literals, patterns } = readers(code());
  return readdirSync(NAMESPACES)
    .filter((id) => statSync(join(NAMESPACES, id)).isDirectory())
    .flatMap((id) => {
      const keys = keysIn(readFileSync(join(NAMESPACES, id, 'index.ts'), 'utf8'));
      return keys.filter((key) => !literals.has(key) && !patterns.some((pattern) => pattern.test(key))).map((key) => ({ namespace: id, key }));
    });
};

/** Deletes `keys` of the namespace `id` from its English table, every locale file and `sources.json`. */
const remove = (id, keys) => {
  const dead = new Set(keys);
  const dir = join(NAMESPACES, id);
  const tables = [join(dir, 'index.ts'), ...walk(join(dir, 'locales')).filter((path) => path.endsWith('.ts'))];
  for (const path of tables) {
    // One entry per line, its value maybe on the next lines: drop from the key's line to the line before the next entry or the table's end.
    const lines = readFileSync(path, 'utf8').split('\n');
    const kept = [];
    let dropping = false;
    for (const line of lines) {
      const entry = /^\s*(['"])([^'"]+)\1\s*:/u.exec(line);
      if (entry !== null) {
        dropping = dead.has(entry[2]);
      } else if (/^\s*(\/\/|\};?$)/u.test(line)) {
        dropping = false;
      }
      if (!dropping) {
        kept.push(line);
      }
    }
    writeFileSync(path, kept.join('\n'));
  }
  const sources = join(dir, 'locales', 'sources.json');
  try {
    const table = JSON.parse(readFileSync(sources, 'utf8'));
    for (const key of dead) {
      delete table[key];
    }
    writeFileSync(sources, `${JSON.stringify(table, null, 1)}\n`);
  } catch (error) {
    if (error.code !== 'ENOENT') {
      throw error;
    }
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const unread = unreadKeys();
  const byNamespace = Map.groupBy(unread, ({ namespace }) => namespace);
  for (const [id, entries] of byNamespace) {
    console.log(`${id}: ${String(entries.length)}`);
    for (const { key } of entries) {
      console.log(`  ${key}`);
    }
    if (process.argv.includes('--remove')) {
      remove(id, entries.map(({ key }) => key));
    }
  }
  console.log(`${String(unread.length)} unread keys${process.argv.includes('--remove') ? ', removed' : ''} (${relative(ROOT, NAMESPACES)})`);
}
