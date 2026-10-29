/**
 * Move message entries between the shell catalogue, the page tables and the
 * namespaces, unchanged.
 *
 *   node scripts/split-catalogue.mjs plan
 *   node scripts/split-catalogue.mjs copy [namespace ...]
 *   node scripts/split-catalogue.mjs fold <table> <namespace> [old=new ...]
 *   node scripts/split-catalogue.mjs prune
 *   node scripts/split-catalogue.mjs move <from> <to> <prefix>
 *
 * `plan` prints where each shell catalogue key belongs, read from the route
 * graph: the shell's own keys stay, a key one route family renders goes to
 * that family's namespace, a key several families render goes to `common`,
 * and a key nothing renders is dead. `copy` copies every planned key into its
 * namespace, creating the namespace's files when they are missing, and leaves
 * the catalogue as it is. `fold` copies a page table (`messages.ts` beside a
 * page) into a namespace, renaming the keys given. `prune` deletes from the
 * catalogue every key a namespace holds, and the dead keys. `move` moves every
 * key of namespace `from` that starts with `prefix` into namespace `to`, for a
 * panel whose words should load with the panel's own chunk.
 *
 * Every entry moves as its source text, with the comment above it, so a hand
 * correction moves with it. Each key's `sources.json` record moves with it as
 * it was, so a key whose English changed before the move is still translated
 * again by `translate-locales.mjs`. Nothing here calls a model.
 */
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const ADMIN = join(dirname(fileURLToPath(import.meta.url)), '..');
const SRC = join(ADMIN, 'src');
const ts = createRequire(join(ADMIN, 'package.json'))('typescript');
const LOCALES = ['es', 'hi', 'mr', 'ml', 'ta', 'kn', 'te', 'ne'];
const NAMES = { es: 'Spanish', hi: 'Hindi', mr: 'Marathi', ml: 'Malayalam', ta: 'Tamil', kn: 'Kannada', te: 'Telugu', ne: 'Nepali' };
const MARKER = '@machine-translated';

// The route family each lazy root belongs to, first match wins. A key the
// editor and the site pages share belongs to `sites`, which loads first.
const FAMILIES = [
  [/^app\/(forgot|password)\.tsx$/, 'account'],
  [/^pages\/(assets|dashboard|jobs|settings|system|users)\//, 'admin'],
  [/^pages\/(items|item-sets|media)\//, 'resources'],
  [/^pages\/sites\/editor\//, 'editor'],
  [/^pages\/sites\//, 'sites'],
  [/^pages\/(fields|standards|vocabularies|properties|resource-classes)\//, 'standards'],
  [/^pages\/item-types\//, 'item-types'],
  [/^pages\/term-lists\//, 'term-lists'],
];
const SHARED = [['sites', 'editor']];

const registry = () => {
  const text = readFileSync(join(SRC, 'i18n/namespaces/registry.ts'), 'utf8');
  return [...text.matchAll(/\{ id: '([^']+)', what: '([^']+)', type: '([^']+)' \}/g)].map(([, id, what, type]) => ({ id, what, type }));
};
const recordOf = (id) => {
  const record = registry().find((entry) => entry.id === id);
  if (!record) throw new Error(`${id}: not in src/i18n/namespaces/registry.ts`);
  return record;
};
const camel = (id) => id.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());

// ---- reading and rewriting one object literal ----

const parse = (path) => ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, path.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
const objectOf = (node) => {
  if (!node) return null;
  if (ts.isObjectLiteralExpression(node)) return node;
  if (ts.isAsExpression(node) || ts.isParenthesizedExpression(node) || ts.isSatisfiesExpression(node)) return objectOf(node.expression);
  return null;
};
/** The object literal a file declares as `const <name>`, or null. */
const findObject = (source, name) => {
  let found = null;
  const visit = (node) => {
    if (!found && ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.name.text === name) found = objectOf(node.initializer);
    if (!found) ts.forEachChild(node, visit);
  };
  visit(source);
  return found;
};
const keyOf = (prop) => (ts.isIdentifier(prop.name) ? prop.name.text : prop.name.text);
/** Each entry: its key, its value, and its source text with the comment above it. */
const entriesOf = (path, name) => {
  const source = parse(path);
  const object = findObject(source, name);
  if (!object) return null;
  return object.properties.map((prop) => {
    if (!ts.isPropertyAssignment(prop)) throw new Error(`${path}: ${name} has a non-literal entry`);
    const full = prop.getFullText(source);
    const trimmed = full.replace(/^[ \t]*\n+/, '').replace(/^\n+/, '');
    return {
      key: keyOf(prop),
      value: prop.initializer.text,
      // Where the key's literal sits in `text`, which starts with one newline.
      nameAt: 1 + prop.name.getStart(source) - prop.getFullStart() - (full.length - trimmed.length),
      nameWidth: prop.name.getWidth(source),
      text: `\n${trimmed}`,
    };
  });
};
/** Replace the object's entries with `entries`, keeping the text around it. */
const writeEntries = (path, name, entries) => {
  const source = parse(path);
  const object = findObject(source, name);
  const text = source.getFullText();
  const body = entries.map((entry) => `${entry.text},`).join('');
  writeFileSync(path, `${text.slice(0, object.getStart(source))}{${body}\n}${text.slice(object.getEnd())}`);
};
const renamed = (entry, key) => {
  if (key === entry.key) return entry;
  const literal = entry.text.slice(entry.nameAt, entry.nameAt + entry.nameWidth);
  const quote = literal.startsWith('"') ? '"' : "'";
  return { ...entry, key, text: `${entry.text.slice(0, entry.nameAt)}${quote}${key}${quote}${entry.text.slice(entry.nameAt + entry.nameWidth)}` };
};

// ---- tables ----

/** A table: where its English lives, and each locale's object. */
const pageTable = (file) => {
  const enFile = resolve(SRC, file);
  const dir = file.includes('describe-messages') ? join(dirname(enFile), 'describe-locales') : join(dirname(enFile), 'locales');
  return {
    name: relative(SRC, enFile),
    english: { path: enFile, name: 'en' },
    locale: (tag) => (existsSync(join(dir, `${tag}.ts`)) ? { path: join(dir, `${tag}.ts`), name: tag } : { path: enFile, name: tag }),
    sources: join(dir, 'sources.json'),
  };
};
const CATALOGUE = {
  name: 'i18n/catalogue.ts',
  english: { path: join(SRC, 'i18n/catalogue.ts'), name: 'en' },
  locale: (tag) => ({ path: join(SRC, `i18n/locales/${tag}.ts`), name: tag }),
  sources: join(SRC, 'i18n/locales/sources.json'),
};
const namespaceDir = (id) => join(SRC, 'i18n/namespaces', id);
const namespaceTable = (id) => ({
  name: `namespace ${id}`,
  english: { path: join(namespaceDir(id), 'index.ts'), name: 'en' },
  locale: (tag) => ({ path: join(namespaceDir(id), 'locales', `${tag}.ts`), name: tag }),
  sources: join(namespaceDir(id), 'locales/sources.json'),
});
const readJson = (path) => (existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : {});
const writeJson = (path, value) => writeFileSync(path, `${JSON.stringify(value, null, 1)}\n`);

// ---- creating a namespace ----

const indexTemplate = ({ id, what, type }) => `/**
 * The ${what}.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under \`locales/\`, fetched the first time a route in
 * that locale reads it. Each locale file is typed \`${type}\`, so a key added
 * here and missing there is a \`tsc\` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
} as const;

/** A complete table of this namespace, in one locale. */
export type ${type} = Table<keyof typeof en>;

export const ${camel(id)} = defineNamespace('${id}', en, {
${LOCALES.map((tag) => `  ${tag}: () => import('./locales/${tag}').then((module) => module.${tag}),`).join('\n')}
});
`;
const localeTemplate = ({ what, type }, tag) => {
  const head =
    tag === 'es'
      ? `/**
 * The Spanish ${what}, written by hand.
 *
 * It is typed \`${type}\`, so a key added to English and not here is a
 * \`tsc\` error.
 */`
      : `/**
 * The ${NAMES[tag]} ${what}.
 *
 * ${MARKER}: written by \`scripts/translate-locales.mjs\` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of ${NAMES[tag]}. Correct
 * a string here; a later run keeps it. It is typed \`${type}\`, so a key
 * added to English and not here is a \`tsc\` error.
 */`;
  return `${head}\nimport type { ${type} } from '../index';\n\nexport const ${tag}: ${type} = {\n};\n`;
};
const ensureNamespace = (id) => {
  const record = recordOf(id);
  const dir = namespaceDir(id);
  mkdirSync(join(dir, 'locales'), { recursive: true });
  if (!existsSync(join(dir, 'index.ts'))) writeFileSync(join(dir, 'index.ts'), indexTemplate(record));
  for (const tag of LOCALES) {
    const path = join(dir, 'locales', `${tag}.ts`);
    if (!existsSync(path)) writeFileSync(path, localeTemplate(record, tag));
  }
  if (!existsSync(join(dir, 'locales/sources.json'))) writeJson(join(dir, 'locales/sources.json'), {});
};

// ---- copying ----

/** Copy `moves` ([old, new] key pairs) from table `from` into namespace `id`. */
const copy = (from, id, moves) => {
  ensureNamespace(id);
  const to = namespaceTable(id);
  const sides = [['en', from.english, to.english], ...LOCALES.map((tag) => [tag, from.locale(tag), to.locale(tag)])];
  let copied = 0;
  for (const [tag, source, target] of sides) {
    const have = entriesOf(source.path, source.name);
    if (!have) throw new Error(`${relative(ADMIN, source.path)}: no const ${source.name}`);
    const byKey = new Map(have.map((entry) => [entry.key, entry]));
    const existing = entriesOf(target.path, target.name);
    const present = new Set(existing.map((entry) => entry.key));
    const added = [];
    for (const [oldKey, newKey] of moves) {
      const entry = byKey.get(oldKey);
      if (!entry) throw new Error(`${from.name} ${tag}: no ${oldKey}`);
      if (present.has(newKey)) {
        const there = existing.find((other) => other.key === newKey);
        if (there.value !== entry.value) throw new Error(`${id} ${tag} ${newKey}: already there with other text`);
        continue;
      }
      added.push(renamed(entry, newKey));
    }
    if (added.length) writeEntries(target.path, target.name, [...existing, ...added]);
    if (tag === 'en') copied = added.length;
  }
  const oldSources = readJson(from.sources);
  const newSources = readJson(to.sources);
  for (const [oldKey, newKey] of moves) {
    if (oldKey in oldSources && !(newKey in newSources)) newSources[newKey] = oldSources[oldKey];
  }
  const order = entriesOf(to.english.path, 'en').map((entry) => entry.key);
  writeJson(to.sources, Object.fromEntries(order.filter((key) => key in newSources).map((key) => [key, newSources[key]])));
  console.log(`${from.name} -> ${id}: ${copied} copied, ${moves.length - copied} already there`);
};

// ---- the route graph ----

const walk = (dir) => readdirSync(dir).flatMap((name) => (statSync(join(dir, name)).isDirectory() ? walk(join(dir, name)) : [join(dir, name)]));
const isTable = (path) => /\/locales\/|describe-locales|\/namespaces\//.test(path) || path.endsWith('i18n/catalogue.ts');
const resolveImport = (from, spec) => {
  if (!spec.startsWith('.')) return null;
  const base = resolve(dirname(from), spec);
  return [base, `${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`].find((path) => existsSync(path) && statSync(path).isFile()) ?? null;
};
const plan = () => {
  const english = entriesOf(CATALOGUE.english.path, 'en').map((entry) => entry.key);
  const keys = new Set(english);
  const files = walk(SRC).filter((path) => /\.(tsx?|json)$/.test(path));
  const graph = new Map();
  const uses = new Map();
  for (const path of files) {
    const text = readFileSync(path, 'utf8');
    const statics = new Set();
    const dynamics = new Set();
    if (!path.endsWith('.json')) {
      for (const match of text.matchAll(/(?:^|\n)\s*(?:import|export)\s+(type\s+)?[^'";]*?from\s+['"]([^'"]+)['"]/g)) {
        if (!match[1]) statics.add(resolveImport(path, match[2]));
      }
      for (const match of text.matchAll(/(?:^|\n)\s*import\s+['"]([^'"]+)['"]/g)) statics.add(resolveImport(path, match[1]));
      for (const match of text.matchAll(/import\(\s*['"]([^'"]+)['"]\s*\)/g)) dynamics.add(resolveImport(path, match[1]));
    }
    statics.delete(null);
    dynamics.delete(null);
    graph.set(path, { statics, dynamics });
    if (isTable(path) || path.endsWith('sources.json')) continue;
    const found = new Set();
    for (const match of text.matchAll(/['"`]([A-Za-z][A-Za-z0-9_.-]*)['"`]/g)) if (keys.has(match[1])) found.add(match[1]);
    // A key built from a prefix, as `locale.${tag}`, uses every key under it.
    for (const match of text.matchAll(/`([A-Za-z][A-Za-z0-9_.]*\.)\$\{/g)) for (const key of keys) if (key.startsWith(match[1])) found.add(key);
    if (found.size) uses.set(path, found);
  }
  const closure = (root) => {
    const seen = new Set();
    const stack = [root];
    while (stack.length) {
      const path = stack.pop();
      if (seen.has(path) || !graph.has(path)) continue;
      seen.add(path);
      for (const next of graph.get(path).statics) stack.push(next);
    }
    return seen;
  };
  const shell = closure(join(SRC, 'main.tsx'));
  const roots = new Set([...graph.values()].flatMap(({ dynamics }) => [...dynamics]).filter((path) => !isTable(path)));
  const families = [...roots].map((root) => {
    const name = relative(SRC, root);
    return [FAMILIES.find(([pattern]) => pattern.test(name))?.[1] ?? name, closure(root)];
  });
  const result = {};
  for (const key of english) {
    const users = [...uses].filter(([, found]) => found.has(key)).map(([path]) => path);
    if (users.some((path) => shell.has(path))) result[key] = 'shell';
    else if (!users.length) result[key] = 'dead';
    else {
      const owners = new Set(families.filter(([, reach]) => users.some((path) => reach.has(path))).map(([family]) => family));
      const lost = [...owners].filter((owner) => !FAMILIES.some(([, family]) => family === owner));
      if (lost.length) throw new Error(`${key}: rendered under ${lost.join(', ')}, a lazy root in no route family; add it to FAMILIES`);
      for (const [kept, dropped] of SHARED) if (owners.size === 2 && owners.has(kept) && owners.has(dropped)) owners.delete(dropped);
      result[key] = owners.size === 1 ? [...owners][0] : 'common';
    }
  }
  return result;
};

// ---- pruning ----

const prune = () => {
  const held = new Set(registry().flatMap(({ id }) => (existsSync(join(namespaceDir(id), 'index.ts')) ? entriesOf(join(namespaceDir(id), 'index.ts'), 'en').map((entry) => entry.key) : [])));
  const placed = plan();
  const gone = (key) => held.has(key) || placed[key] === 'dead';
  const unplaced = Object.entries(placed).filter(([key, where]) => where !== 'shell' && where !== 'dead' && !held.has(key));
  if (unplaced.length) throw new Error(`not in their namespace yet; run copy first: ${unplaced.map(([key]) => key).join(' ')}`);
  for (const [tag, side] of [['en', CATALOGUE.english], ...LOCALES.map((tag) => [tag, CATALOGUE.locale(tag)])]) {
    const entries = entriesOf(side.path, side.name);
    writeEntries(side.path, side.name, entries.filter((entry) => !gone(entry.key)));
    if (tag === 'en') console.log(`catalogue: ${entries.filter((entry) => gone(entry.key)).length} removed, ${entries.filter((entry) => !gone(entry.key)).length} kept`);
  }
  const sources = readJson(CATALOGUE.sources);
  writeJson(CATALOGUE.sources, Object.fromEntries(Object.entries(sources).filter(([key]) => !gone(key))));
};

// ---- moving between namespaces ----

/** Move the keys of namespace `fromId` under `prefix` into namespace `id`. */
const move = (fromId, id, prefix) => {
  const from = namespaceTable(fromId);
  const keys = entriesOf(from.english.path, 'en').map((entry) => entry.key).filter((key) => key.startsWith(prefix));
  if (!keys.length) {
    console.log(`${fromId}: no key starts with ${prefix}`);
    return;
  }
  copy(from, id, keys.map((key) => [key, key]));
  const gone = new Set(keys);
  for (const side of [from.english, ...LOCALES.map((tag) => from.locale(tag))]) {
    writeEntries(side.path, side.name, entriesOf(side.path, side.name).filter((entry) => !gone.has(entry.key)));
  }
  writeJson(from.sources, Object.fromEntries(Object.entries(readJson(from.sources)).filter(([key]) => !gone.has(key))));
  console.log(`${fromId}: ${keys.length} removed`);
};

// ---- commands ----

const [command, ...args] = process.argv.slice(2);
if (command === 'plan') {
  const placed = plan();
  const counts = {};
  for (const where of Object.values(placed)) counts[where] = (counts[where] ?? 0) + 1;
  for (const [key, where] of Object.entries(placed)) console.log(`${where}\t${key}`);
  console.log(JSON.stringify(counts));
} else if (command === 'copy') {
  const placed = plan();
  const ids = args.length ? args : [...new Set(Object.values(placed))].filter((where) => where !== 'shell' && where !== 'dead');
  for (const id of ids) {
    copy(CATALOGUE, id, Object.entries(placed).filter(([, where]) => where === id).map(([key]) => [key, key]));
  }
} else if (command === 'fold') {
  const [file, id, ...renames] = args;
  const table = pageTable(file);
  const rename = new Map(renames.map((pair) => pair.split('=')));
  const keys = entriesOf(table.english.path, 'en').map((entry) => entry.key);
  copy(table, id, keys.map((key) => [key, rename.get(key) ?? key]));
} else if (command === 'prune') {
  prune();
} else if (command === 'move') {
  const [from, id, prefix] = args;
  move(from, id, prefix);
} else {
  console.error('usage: split-catalogue.mjs plan | copy [namespace ...] | fold <table> <namespace> [old=new ...] | prune | move <from> <to> <prefix>');
  process.exitCode = 2;
}
