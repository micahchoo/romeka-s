/**
 * The lessons' messages. They load with the chunk of the page that shows a
 * lesson, never with the main catalogue, which every first paint pays for
 * (`perf/budget.json`). English is here; every other locale is its own chunk
 * under `locales/`. Each table is typed against `en`, so a key added in English
 * and missing elsewhere is a `tsc` error, and a locale with no loader is one too.
 */
import { useEffect, useMemo, useState } from 'react';
import { useLocale } from '../../i18n';
import type { Locale, MessageParams } from '../../i18n';

export const en = {
  'lesson.label': 'Lesson: {title}',
  'lesson.step': 'Step {number} of {count}: {rung}',
  'lesson.gotIt': 'Got it',
  'lesson.recall': 'Show the lesson: {title}',
  'lesson.chip': 'Lesson',
  'lesson.learnMore': 'Read about {standard}',
  'rung.describe': 'Describe',
  'rung.kinds': 'Kinds of things',
  'rung.fixedValues': 'Fixed values',
  'rung.linkedData': 'Linked data',
  'rung.makingStandards': 'Making standards',
  'standard.dublinCore': 'Dublin Core',
  'standard.rdfClasses': 'classes in RDF Schema',
  'standard.linkedData': 'linked data',
  'standard.localVocabulary': 'vocabularies in Omeka S',
  'standard.skos': 'controlled vocabularies in SKOS',
  'standard.skosMapping': 'matching terms across vocabularies in SKOS',
  'standard.coolUris': 'web addresses that do not change',
  'term.title': 'Standard terms',
  'term.body':
    'The code under each field, such as dcterms:creator, is its standard term from Dublin Core. Values are saved under that term, so another archive that reads Dublin Core knows what each value is without asking you.',
  'rename.title': 'Names and terms',
  'rename.body':
    'You changed the name people see on the form. The value is still saved under the same standard term, so search, exports and other archives read it as before.',
  'fieldsPage.title': 'Shared fields',
  'fieldsPage.body':
    'Every field here is a term from a shared vocabulary, most often Dublin Core. When two archives both save a creator as dcterms:creator, their records can be searched and merged together.',
  'required.title': 'Required fields',
  'required.body':
    'This item type marks the field as required, so every item of this type has a value here. A collection where every record has it can be sorted and searched with no gaps.',
  'typeClass.title': 'Standard classes',
  'typeClass.body':
    'The name is for people. The standard class, such as bibo:Letter, tells software what kind of thing each item is, so a search for that class, here or in another archive, finds every item of this type.',
  'typePick.title': 'Item types',
  'typePick.body':
    'A type lays out the fields for one kind of thing and saves the item with its standard class. Items of one type share the same fields, so they are easy to compare and to find together.',
  'standards.title': 'Linked data',
  'standards.body':
    'These are the vocabularies behind every field and class. Each term has a web address, such as http://purl.org/dc/terms/title, so a value saved here means the same thing wherever it is read.',
  'localField.title': 'Your own vocabulary',
  'localField.body':
    'Its name starts with local: because no shared standard had this field, so it is a term in this archive’s own vocabulary. Its address is on this site, so other archives can still read the value and look up what it means.',
  'termMerge.title': 'One spelling',
  'termMerge.body':
    'Merging changes every value written another way into the one spelling you keep. With one spelling for each thing, a search or a list of values finds every record, not only the ones typed the same way.',
  'tagPromote.title': 'A controlled vocabulary',
  'tagPromote.body':
    'Promoting makes the chosen tags the words of a list, and the Subject field of these item types then offers that list. People pick a word from it instead of typing one, so each thing keeps one spelling.',
  'authorityLink.title': 'Links to an authority',
  'authorityLink.body':
    'An exact match says that this term and a record in an authority, such as GeoNames or Wikidata, name the same thing. The published list carries the link as skos:exactMatch, so software that knows that record can join your records to everything else linked to it.',
  'publish.title': 'Addresses that last',
  'publish.body':
    'Publishing gives each term a web address that never changes, and later versions can add terms but never remove one. Records here and in other archives can cite those addresses and trust that the links keep working.',
  'switch.label': 'Show lessons',
  'switch.hint':
    'Short notes that explain the standard behind an action, the first time you take it.',
} as const;

export type LessonsMessageKey = keyof typeof en;

/** A complete lessons table: every message id, no fallback at read time. */
export type LessonsMessages = Readonly<Record<LessonsMessageKey, string>>;

const LESSONS_LOADERS: Readonly<Record<Locale, () => Promise<LessonsMessages>>> = {
  en: () => Promise.resolve(en),
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
};

const loaded = new Map<Locale, LessonsMessages>([['en', en]]);

/** A lookup into the lessons table. */
export type LessonsText = (key: LessonsMessageKey, params?: MessageParams) => string;

const fill = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z]+)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/**
 * The active locale's lookup, or `null` until its table arrives. A lesson
 * never holds up its page, so a caller renders nothing while this is `null`.
 */
export const useLessonsText = (): LessonsText | null => {
  const locale = useLocale();
  const [table, setTable] = useState<LessonsMessages | null>(() => loaded.get(locale) ?? null);
  useEffect(() => {
    const cached = loaded.get(locale);
    if (cached !== undefined) {
      setTable(cached);
      return undefined;
    }
    let current = true;
    void LESSONS_LOADERS[locale]().then((messages) => {
      loaded.set(locale, messages);
      if (current) {
        setTable(messages);
      }
    });
    return () => {
      current = false;
    };
  }, [locale]);
  return useMemo(
    () =>
      table === null
        ? null
        : (key: LessonsMessageKey, params?: MessageParams): string => fill(table[key], params),
    [table],
  );
};
