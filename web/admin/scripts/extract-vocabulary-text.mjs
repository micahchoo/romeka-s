/**
 * Write the English and Spanish text of the four built-in vocabularies from
 * upstream's gettext catalogues.
 *
 *   node scripts/extract-vocabulary-text.mjs <omeka-s checkout>
 *
 * Upstream's `application/language/template.pot` marks each vocabulary, class
 * and property label and comment with an extracted comment such as
 * `#. Property label for Dublin Core:title`. That comment names the term, so
 * this script keys each text by term (`dcterms:title`, `bibo:Document`, or the
 * vocabulary prefix), never by its English. The pot merges a msgid that the
 * application also uses (`Title`) and drops its vocabulary comment, so a term
 * the pot does not name takes its English from the install seed,
 * `crates/omeka-cli/data/default-vocabularies.json`.
 *
 * Spanish is `es.po`'s msgstr for each English msgid. A term whose msgid has
 * no msgstr keeps the translation already in `es.json`, which is written by
 * hand, and the script lists every such term. A term with neither stops it.
 *
 * The files are `src/i18n/locales/vocabulary/{labels,comments}/<tag>.json`.
 * The other locales are filled by `scripts/translate-locales.mjs`.
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ADMIN = join(dirname(fileURLToPath(import.meta.url)), '..');
const SEED = join(ADMIN, '../../crates/omeka-cli/data/default-vocabularies.json');
const OUT = join(ADMIN, 'src/i18n/locales/vocabulary');

const upstream = process.argv[2];
if (!upstream) throw new Error('usage: extract-vocabulary-text.mjs <omeka-s checkout>');

/** The entries of a `.po` or `.pot` file: extracted comments, msgid, msgstr. */
const parsePo = (path) => {
  const entries = [];
  let entry = null;
  let field = null;
  const flush = () => {
    if (entry?.msgid !== undefined) entries.push(entry);
    entry = null;
  };
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.trim() === '') {
      flush();
      continue;
    }
    entry ??= { comments: [] };
    const keyword = line.match(/^(msgctxt|msgid_plural|msgid|msgstr(?:\[\d\])?)\s+(".*")$/);
    if (line.startsWith('#.')) entry.comments.push(line.slice(2).trim());
    else if (keyword) {
      field = keyword[1];
      entry[field] = JSON.parse(keyword[2]);
    } else if (line.startsWith('"') && field) entry[field] += JSON.parse(line);
  }
  flush();
  return entries;
};

const seed = JSON.parse(readFileSync(SEED, 'utf8')).vocabularies;
const prefixOf = new Map(seed.map((vocabulary) => [vocabulary.label, vocabulary.prefix]));
const language = join(upstream, 'application/language');

// The pot's English per term, from its extracted comments.
const potEnglish = { label: new Map(), comment: new Map() };
for (const entry of parsePo(join(language, 'template.pot'))) {
  for (const comment of entry.comments) {
    const term = comment.match(/^(?:Property|Class) (label|comment) for (.+):([^:]+)$/);
    const vocabulary = comment.match(/^Vocabulary (label|comment) for (.+)$/);
    const [what, key] = term
      ? [term[1], `${prefixOf.get(term[2])}:${term[3]}`]
      : vocabulary
        ? [vocabulary[1], prefixOf.get(vocabulary[2])]
        : [null, null];
    if (!what) continue;
    if (key.startsWith('undefined')) throw new Error(`no seed vocabulary for "${comment}"`);
    potEnglish[what].set(key, entry.msgid);
  }
}

// Seed order: each vocabulary, then its classes, then its properties; the
// terms only the pot names follow their vocabulary.
const english = { label: {}, comment: {} };
for (const vocabulary of seed) {
  const keys = [
    vocabulary.prefix,
    ...[...vocabulary.classes, ...vocabulary.properties].map((term) => `${vocabulary.prefix}:${term.local_name}`),
  ];
  const seeded = new Map([
    [vocabulary.prefix, vocabulary],
    ...[...vocabulary.classes, ...vocabulary.properties].map((term) => [
      `${vocabulary.prefix}:${term.local_name}`,
      term,
    ]),
  ]);
  for (const what of ['label', 'comment']) {
    const extra = [...potEnglish[what].keys()].filter(
      (key) => !seeded.has(key) && (key === vocabulary.prefix || key.startsWith(`${vocabulary.prefix}:`)),
    );
    for (const key of [...keys, ...extra]) {
      const text = potEnglish[what].get(key) ?? seeded.get(key)?.[what];
      if (text) english[what][key] = text;
    }
  }
}

const spanish = new Map(
  parsePo(join(language, 'es.po'))
    .filter((entry) => entry.msgctxt === undefined && entry.msgstr)
    .map((entry) => [entry.msgid, entry.msgstr]),
);

const NOTES = {
  en: 'The English text of the built-in vocabularies, keyed by term. Written by scripts/extract-vocabulary-text.mjs from upstream application/language/template.pot and the install seed.',
  es: "The Spanish text of the built-in vocabularies, keyed by term. Written by scripts/extract-vocabulary-text.mjs from upstream application/language/es.po; a term es.po leaves untranslated keeps the hand-written text here.",
};

const write = (kind, tag, terms) =>
  writeFileSync(join(OUT, kind, `${tag}.json`), `${JSON.stringify({ note: NOTES[tag], terms }, null, 1)}\n`);

for (const [what, kind] of [
  ['label', 'labels'],
  ['comment', 'comments'],
]) {
  const path = join(OUT, kind, 'es.json');
  const kept = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')).terms : {};
  const terms = {};
  const byHand = [];
  const missing = [];
  for (const [key, text] of Object.entries(english[what])) {
    const translated = spanish.get(text);
    if (translated) terms[key] = translated;
    else if (kept[key]) {
      terms[key] = kept[key];
      byHand.push(key);
    } else missing.push(`${key} ${JSON.stringify(text)}`);
  }
  if (missing.length) throw new Error(`es.po has no msgstr for these ${what}s; write each in ${path}:\n${missing.join('\n')}`);
  write(kind, 'en', english[what]);
  write(kind, 'es', terms);
  console.log(
    `${kind}: ${String(Object.keys(english[what]).length)} terms, ${String(byHand.length)} Spanish by hand${byHand.length ? `: ${byHand.join(' ')}` : ''}`,
  );
}
