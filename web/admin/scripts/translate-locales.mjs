/**
 * Fill the machine-translated catalogues with the English keys they lack.
 *
 *   bun scripts/translate-locales.mjs [tag ...]
 *
 * With no tags it fills every catalogue marked `@machine-translated`. The
 * locale file is the store: a run translates only missing keys and drops keys
 * English no longer has, so a hand correction in the file is never undone.
 * Each table's `sources.json` records the English every translation was made
 * from, so a key whose English has since changed is translated again. Only a
 * run over every machine-translated locale (no tags) updates that record.
 *
 * The model is sarvam-30b behind llama.cpp (`SARVAM_URL`, default
 * http://localhost:8086). The prompt and the answer checks are md-translator's
 * (`TRANSLATOR_DIR`): its parallel-text prompt leaves the model no chat turn to
 * think in, which the chat endpoint cannot switch off. A string whose best of
 * four attempts still fails a check is written and printed for review.
 *
 * It runs under bun, not node: it imports TypeScript from the catalogues and
 * from md-translator.
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ADMIN = join(dirname(fileURLToPath(import.meta.url)), '..');
// Every English table and where its translations live. The main catalogue's
// files decide which locales are machine-translated; each other table follows.
const TABLES = [
  { what: 'catalogue', source: 'src/i18n/catalogue.ts', dir: 'src/i18n/locales', type: 'Catalogue', from: '../catalogue' },
  {
    what: 'Theme page messages',
    source: 'src/pages/sites/themeEditor/messages.ts',
    dir: 'src/pages/sites/themeEditor/locales',
    type: 'ThemeMessages',
    from: '../messages',
  },
  {
    what: 'page editor field messages',
    source: 'src/pages/sites/editor/messages.ts',
    dir: 'src/pages/sites/editor/locales',
    type: 'EditorMessages',
    from: '../messages',
  },
  {
    what: 'Standards page messages',
    source: 'src/pages/standards/messages.ts',
    dir: 'src/pages/standards/locales',
    type: 'StandardsMessages',
    from: '../messages',
  },
  {
    what: 'Term lists messages',
    source: 'src/pages/term-lists/messages.ts',
    dir: 'src/pages/term-lists/locales',
    type: 'TermListMessages',
    from: '../messages',
  },
  {
    what: 'item type pages messages',
    source: 'src/pages/item-types/messages.tsx',
    dir: 'src/pages/item-types/locales',
    type: 'ItemTypesMessages',
    from: '../messages',
  },
  {
    what: 'describe strings',
    source: 'src/app/describe-messages.ts',
    dir: 'src/app/describe-locales',
    type: 'DescribeMessages',
    from: '../describe-messages',
  },
];
const TRANSLATOR = process.env.TRANSLATOR_DIR ?? '/mnt/Ghar/2TA/DevStuff/notebook/translator';
const SERVER = process.env.SARVAM_URL ?? 'http://localhost:8086';
const MARKER = '@machine-translated';
const RETRIES = 3;
// Earlier answers shown to the model, so a term keeps one translation.
const CONTEXT = 8;
// Names a reader must recognise as written: an answer that drops or transliterates one is flagged.
const NAMES = ['Romeka S', 'Omeka S', 'IIIF', 'URL', 'HTML', 'CSS', 'JSON', 'API'];
const PREAMBLE =
  'The following are English interface strings from Romeka S, a web admin app for digital collections ' +
  '(items, item sets, media, sites, vocabularies, resource templates), with faithful, complete {L} translations ' +
  'in the formal style of software menus, buttons and messages. Every word is translated. ' +
  `Placeholders in braces such as {count}, square brackets, punctuation and the names ${NAMES.join(', ')} are kept unchanged, in Latin letters.`;

if (!existsSync(join(TRANSLATOR, 'src/prompt.ts'))) {
  throw new Error(`md-translator not found at ${TRANSLATOR}; set TRANSLATOR_DIR`);
}
const { LANGUAGES } = await import(join(TRANSLATOR, 'src/languages.ts'));
const { buildPrompt, cleanOutput, stopFor } = await import(join(TRANSLATOR, 'src/prompt.ts'));
const { check } = await import(join(TRANSLATOR, 'src/checks.ts'));

const placeholders = (text) => (text.match(/\{[a-zA-Z]+\}/g) ?? []).sort().join();
// A string with no word outside its placeholders, names and acronyms, as `{field} URI` or `Omeka S`, is the same in every language.
const wordless = (text) =>
  !/[A-Za-z]/.test(NAMES.reduce((rest, name) => rest.replaceAll(name, ''), text).replace(/\{[a-zA-Z]+\}/g, '').replace(/\b[A-Z]{2,}\b/g, ''));
// Endonyms are the same in every catalogue, and English holds them.
const endonym = (key) => key.startsWith('locale.') && key !== 'locale.label';

const quote = (text) => `'${text.replaceAll('\\', '\\\\').replaceAll("'", "\\'").replaceAll('\n', '\\n')}'`;

async function complete(prompt, temperature, seed, maxTokens, stop) {
  const response = await fetch(`${SERVER}/v1/completions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ prompt, temperature, seed, max_tokens: maxTokens, stop, cache_prompt: true }),
  });
  if (!response.ok) throw new Error(`${SERVER}: ${response.status} ${await response.text()}`);
  return (await response.json()).choices[0].text;
}

/** The best of up to four answers, and the checks it still fails. */
async function translate(language, source, history) {
  const prompt = buildPrompt({
    language,
    preamble: PREAMBLE,
    examples: language.examples,
    context: history.slice(-CONTEXT),
    source,
  });
  let best = null;
  for (let attempt = 0; attempt <= RETRIES; attempt++) {
    const raw = await complete(prompt, attempt ? 0.6 : 0, attempt, Math.max(256, source.length * 4), stopFor(language));
    const output = cleanOutput(raw, language);
    const flags = check(source, output, language, []);
    if (placeholders(source) !== placeholders(output)) flags.push('placeholders');
    if (NAMES.some((name) => source.includes(name) && !output.includes(name))) flags.push('names');
    if (!best || flags.length < best.flags.length) best = { output, flags };
    if (!flags.length) break;
  }
  // cleanOutput trims, but ' (HTTP {status})' is appended to text and needs its space back.
  const [, lead, , trail] = source.match(/^(\s*)([\s\S]*?)(\s*)$/);
  return { output: lead + best.output.trim() + trail, flags: best.flags };
}

function write(path, header, table, tag, en, catalogue) {
  const entries = Object.keys(en)
    .filter((key) => key in catalogue)
    .map((key) => {
      const line = `  ${quote(key)}: ${quote(catalogue[key])},`;
      return line.length <= 100 ? line : `  ${quote(key)}:\n    ${quote(catalogue[key])},`;
    });
  writeFileSync(path, `${header}export const ${tag}: ${table.type} = {\n${entries.join('\n')}\n};\n`);
}

const header = (table, name) => `/**
 * The ${name} ${table.what}.
 *
 * ${MARKER}: written by \`scripts/translate-locales.mjs\` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of ${name}. Correct
 * a string here; a later run keeps it. It is typed \`${table.type}\`, so a key
 * added to English and not here is a \`tsc\` error.
 */
import type { ${table.type} } from '${table.from}';

`;

const tags = process.argv.slice(2);
const targets = tags.length
  ? tags
  : Object.keys(LANGUAGES).filter((tag) => {
      const path = join(ADMIN, TABLES[0].dir, `${tag}.ts`);
      return existsSync(path) && readFileSync(path, 'utf8').includes(MARKER);
    });

let flaggedTotal = 0;
for (const table of TABLES) {
  const { en } = await import(join(ADMIN, table.source));
  const sourcesPath = join(ADMIN, table.dir, 'sources.json');
  // With no record yet, every translation present is taken as made from today's English.
  const sources = existsSync(sourcesPath) ? JSON.parse(readFileSync(sourcesPath, 'utf8')) : en;
  const changed = new Set(Object.keys(en).filter((key) => key in sources && sources[key] !== en[key]));
  for (const tag of targets) {
    const language = LANGUAGES[tag];
    if (!language || tag === 'en') throw new Error(`${tag}: md-translator has no such target language`);
    const path = join(ADMIN, table.dir, `${tag}.ts`);
    const text = existsSync(path) ? readFileSync(path, 'utf8') : '';
    if (text && !text.includes(MARKER)) throw new Error(`${path} is hand-written; this script fills only ${MARKER} files`);
    const existing = text ? (await import(path))[tag] : {};
    const kept = text ? text.slice(0, text.indexOf('export const')) : header(table, language.name);

    const catalogue = {};
    const missing = [];
    for (const key of Object.keys(en)) {
      if (endonym(key)) catalogue[key] = en[key];
      else if (key in existing && !changed.has(key)) catalogue[key] = existing[key];
      else if (wordless(en[key])) catalogue[key] = en[key];
      else missing.push(key);
    }
    const history = Object.keys(en)
      .filter((key) => key in existing && !endonym(key) && !changed.has(key))
      .map((key) => [en[key], existing[key]]);

    for (const key of missing) {
      const { output, flags } = await translate(language, en[key], history);
      catalogue[key] = output;
      if (flags.length) {
        flaggedTotal++;
        console.log(`${tag} ${key} [${flags.join(',')}] ${JSON.stringify(en[key])} -> ${JSON.stringify(output)}`);
      } else history.push([en[key], output]);
      // Every answer is saved at once, so an interrupted run resumes where it stopped.
      write(path, kept, table, tag, en, catalogue);
    }
    write(path, kept, table, tag, en, catalogue);
    console.log(`${tag} ${table.what}: ${missing.length} translated`);
  }
  if (tags.length) {
    if (changed.size) console.log(`${table.what}: sources.json not updated, because only some locales ran`);
  } else {
    writeFileSync(sourcesPath, `${JSON.stringify(en, null, 1)}\n`);
  }
}
if (flaggedTotal) console.log(`${flaggedTotal} flagged: read each against its English and correct the file.`);
