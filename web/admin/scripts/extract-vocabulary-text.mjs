/**
 * Write the English and Spanish text of the four built-in vocabularies.
 *
 *   node scripts/extract-vocabulary-text.mjs <omeka-s checkout>
 *
 * English is the text a fresh install stores, keyed by term (`dcterms:title`,
 * `bibo:Document`, or the vocabulary prefix), in install order. Its source is
 * `crates/omeka-cli/tests/fixtures/oracle-default-vocabularies.json`, a fresh
 * upstream install read back through its API; `romeka install` is held to it
 * by that crate's `default_vocabularies` test. The translation guard compares
 * a stored text against this English, so it must be the stored text exactly.
 *
 * Spanish is upstream `application/language/es.po`'s msgstr for each English
 * msgid. A term whose msgid has no msgstr keeps the translation already in
 * `es.json`, which is written by hand, and the script lists every such term.
 * A term with neither stops it.
 *
 * The files are `src/i18n/locales/vocabulary/{labels,comments}/<tag>.json`.
 * The other locales are filled by `scripts/translate-locales.mjs`.
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ADMIN = join(dirname(fileURLToPath(import.meta.url)), '..');
const INSTALLED = join(ADMIN, '../../crates/omeka-cli/tests/fixtures/oracle-default-vocabularies.json');
const OUT = join(ADMIN, 'src/i18n/locales/vocabulary');

const upstream = process.argv[2];
if (!upstream) throw new Error('usage: extract-vocabulary-text.mjs <omeka-s checkout>');

/** The entries of a `.po` file: msgctxt, msgid, msgstr. */
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
    entry ??= {};
    const keyword = line.match(/^(msgctxt|msgid_plural|msgid|msgstr(?:\[\d\])?)\s+(".*")$/);
    if (keyword) {
      field = keyword[1];
      entry[field] = JSON.parse(keyword[2]);
    } else if (line.startsWith('"') && field) entry[field] += JSON.parse(line);
  }
  flush();
  return entries;
};

const english = { label: {}, comment: {} };
for (const vocabulary of JSON.parse(readFileSync(INSTALLED, 'utf8'))) {
  const rows = [
    [vocabulary.prefix, vocabulary],
    ...[...vocabulary.classes, ...vocabulary.properties].map((term) => [`${vocabulary.prefix}:${term.local_name}`, term]),
  ];
  for (const what of ['label', 'comment']) for (const [key, row] of rows) if (row[what]) english[what][key] = row[what];
}

const spanish = new Map(
  parsePo(join(upstream, 'application/language/es.po'))
    .filter((entry) => entry.msgctxt === undefined && entry.msgstr)
    .map((entry) => [entry.msgid, entry.msgstr]),
);

const NOTES = {
  en: 'The English text of the built-in vocabularies, keyed by term. Written by scripts/extract-vocabulary-text.mjs from the text a fresh upstream install stores.',
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
