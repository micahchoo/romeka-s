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
 * The public site's strings are one more table, kept as JSON under
 * `crates/omeka-site/src/i18n/locales/`, because Rust reads them. A string the
 * admin already translated takes the admin's translation there, so the admin
 * and the site name a thing alike.
 *
 * It runs under bun, not node: it imports TypeScript from the catalogues and
 * from md-translator.
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ADMIN = join(dirname(fileURLToPath(import.meta.url)), '..');
// The key a JSON table keeps its note under; it is not a message.
const NOTE = '//';
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
    what: 'vocabulary import messages',
    source: 'src/pages/vocabularies/messages.tsx',
    dir: 'src/pages/vocabularies/locales',
    type: 'ImportMessages',
    from: '../messages',
  },
  {
    what: 'item form messages',
    source: 'src/pages/items/messages.ts',
    dir: 'src/pages/items/locales',
    type: 'ItemMessages',
    from: '../messages',
  },
  {
    what: 'Jobs page messages',
    source: 'src/pages/jobs/messages.ts',
    dir: 'src/pages/jobs/locales',
    type: 'JobsMessages',
    from: '../messages',
  },
  {
    what: 'describe strings',
    source: 'src/app/describe-messages.ts',
    dir: 'src/app/describe-locales',
    type: 'DescribeMessages',
    from: '../describe-messages',
  },
  {
    what: 'lessons messages',
    source: 'src/app/lessons/messages.ts',
    dir: 'src/app/lessons/locales',
    type: 'LessonsMessages',
    from: '../messages',
  },
  {
    // The server's error patterns: the Rust registry generates the English.
    what: 'server messages',
    source: 'src/generated/serverMessages.json',
    english: (module) => module.default.messages,
    dir: 'src/i18n/locales',
    file: (tag) => `${tag}.server.ts`,
    name: (tag) => `${tag}Server`,
    sources: 'sources.server.json',
    type: 'ServerCatalogue',
    from: '../serverMessages',
  },
  {
    // The public site's catalogue: Rust reads the JSON with `include_str!`, and
    // a test generates the English from the Rust statics. It runs last, so a
    // string the admin already translated keeps the admin's wording.
    what: 'public site strings',
    source: '../../crates/omeka-site/src/i18n/locales/en.json',
    english: (module) => Object.fromEntries(Object.entries(module.default).filter(([key]) => key !== NOTE)),
    dir: '../../crates/omeka-site/src/i18n/locales',
    file: (tag) => `${tag}.json`,
    json: true,
    reuse: true,
  },
];
// A table names its English export `en`, a locale file `<tag>.ts` exporting `<tag>`, unless it says otherwise.
const englishOf = (table, module) => (table.english ? table.english(module) : module.en);
const fileOf = (table, tag) => (table.file ? table.file(tag) : `${tag}.ts`);
const nameOf = (table, tag) => (table.name ? table.name(tag) : tag);
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

// The site's strings take printf's `%s` where the admin's take braces.
const placeholders = (text) => (text.match(/\{[a-zA-Z]+\}|%s/g) ?? []).sort().join();
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
    let raw = '';
    try {
      raw = await complete(prompt, attempt ? 0.6 : 0, attempt, Math.max(256, source.length * 4), stopFor(language));
    } catch (error) {
      // llama.cpp sometimes refuses its own output with a 500; that attempt is simply lost.
      console.log(`  attempt ${String(attempt)} failed: ${String(error)}`);
    }
    const output = cleanOutput(raw, language);
    // The earlier pairs let md-translator flag an answer copied from another source string.
    const flags = check(source, output, language, history);
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
  if (table.json) {
    const ordered = Object.fromEntries(Object.keys(en).filter((key) => key in catalogue).map((key) => [key, catalogue[key]]));
    writeFileSync(path, `${JSON.stringify({ [NOTE]: header, ...ordered }, null, 2)}\n`);
    return;
  }
  const entries = Object.keys(en)
    .filter((key) => key in catalogue)
    .map((key) => {
      const line = `  ${quote(key)}: ${quote(catalogue[key])},`;
      return line.length <= 100 ? line : `  ${quote(key)}:\n    ${quote(catalogue[key])},`;
    });
  writeFileSync(path, `${header}export const ${nameOf(table, tag)}: ${table.type} = {\n${entries.join('\n')}\n};\n`);
}

// A JSON file has no comment, so its note is a string under NOTE.
const jsonHeader = (name) =>
  `${MARKER}: written by scripts/translate-locales.mjs with sarvam-30b and md-translator's checks, and not yet ` +
  `reviewed by a reader of ${name}. Correct a string here; a later run keeps it. A missing or extra key fails ` +
  'every_shipped_locale_parses in omeka-site.';

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
      const path = join(ADMIN, TABLES[0].dir, fileOf(TABLES[0], tag));
      return existsSync(path) && readFileSync(path, 'utf8').includes(MARKER);
    });

let flaggedTotal = 0;
// Per locale, each English string an earlier table translated, and its translation.
const translated = Object.fromEntries(targets.map((tag) => [tag, new Map()]));
for (const table of TABLES) {
  const en = englishOf(table, await import(join(ADMIN, table.source)));
  const sourcesPath = join(ADMIN, table.dir, table.sources ?? 'sources.json');
  // With no record yet, every translation present is taken as made from today's English.
  const sources = existsSync(sourcesPath) ? JSON.parse(readFileSync(sourcesPath, 'utf8')) : en;
  const changed = new Set(Object.keys(en).filter((key) => key in sources && sources[key] !== en[key]));
  for (const tag of targets) {
    const language = LANGUAGES[tag];
    if (!language || tag === 'en') throw new Error(`${tag}: md-translator has no such target language`);
    const path = join(ADMIN, table.dir, fileOf(table, tag));
    const text = existsSync(path) ? readFileSync(path, 'utf8') : '';
    if (text && !text.includes(MARKER)) throw new Error(`${path} is hand-written; this script fills only ${MARKER} files`);
    let existing = {};
    let kept = header(table, language.name);
    if (table.json) {
      if (text) ({ [NOTE]: kept, ...existing } = JSON.parse(text));
      else kept = jsonHeader(language.name);
    } else if (text) {
      existing = (await import(path))[nameOf(table, tag)];
      kept = text.slice(0, text.indexOf('export const'));
    }

    const catalogue = {};
    const missing = [];
    for (const key of Object.keys(en)) {
      if (endonym(key)) catalogue[key] = en[key];
      else if (key in existing && !changed.has(key)) catalogue[key] = existing[key];
      else if (table.reuse && translated[tag].has(en[key])) catalogue[key] = translated[tag].get(en[key]);
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
    for (const key of Object.keys(en)) {
      if (!endonym(key) && !translated[tag].has(en[key])) translated[tag].set(en[key], catalogue[key]);
    }
  }
  if (tags.length) {
    if (changed.size) console.log(`${table.what}: sources.json not updated, because only some locales ran`);
  } else {
    writeFileSync(sourcesPath, `${JSON.stringify(en, null, 1)}\n`);
  }
}
if (flaggedTotal) console.log(`${flaggedTotal} flagged: read each against its English and correct the file.`);
