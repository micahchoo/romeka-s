/**
 * Every lesson the admin can show, as data.
 *
 * A lesson teaches the standard behind one action, at the moment of that
 * action. The lessons climb one ladder (`RUNGS`): describe a thing, say what
 * kind it is, fix its values, link them, then make standards of your own.
 *
 * A lesson id is stored in each user's `romeka_lessons_seen` setting, so an id
 * never changes once it ships.
 */
import type { LessonsMessageKey } from './messages';

/** The ladder, in the order a beginner climbs it. */
export const RUNGS = ['describe', 'kinds', 'fixedValues', 'linkedData', 'makingStandards'] as const;

export type Rung = (typeof RUNGS)[number];

/** Each standard a lesson names, with its own page for "learn more". */
export const STANDARDS = {
  dublinCore: 'https://www.dublincore.org/specifications/dublin-core/dcmi-terms/',
  rdfClasses: 'https://www.w3.org/TR/rdf-schema/#ch_classes',
  linkedData: 'https://www.w3.org/DesignIssues/LinkedData.html',
  localVocabulary: 'https://omeka.org/s/docs/user-manual/content/vocabularies/',
  skos: 'https://www.w3.org/TR/skos-primer/',
  skosMapping: 'https://www.w3.org/TR/skos-reference/#mapping',
  coolUris: 'https://www.w3.org/Provider/Style/URI',
} as const;

export type StandardId = keyof typeof STANDARDS;

interface LessonText {
  readonly title: LessonsMessageKey;
  readonly body: LessonsMessageKey;
}

interface Lesson {
  readonly rung: Rung;
  /** The UI moment that shows the lesson, for the next person who places one. */
  readonly trigger: string;
  readonly standard: StandardId;
  readonly text: LessonText;
}

export const LESSONS = {
  term: {
    rung: 'describe',
    trigger: 'Item types: the first field row shows its standard term.',
    standard: 'dublinCore',
    text: { title: 'term.title', body: 'term.body' },
  },
  rename: {
    rung: 'describe',
    trigger: 'Item types: the user types a new name for a field.',
    standard: 'dublinCore',
    text: { title: 'rename.title', body: 'rename.body' },
  },
  'fields-page': {
    rung: 'describe',
    trigger: 'Fields: the page opens.',
    standard: 'dublinCore',
    text: { title: 'fieldsPage.title', body: 'fieldsPage.body' },
  },
  required: {
    rung: 'describe',
    trigger: 'Item form: the first field the type marks as required.',
    standard: 'dublinCore',
    text: { title: 'required.title', body: 'required.body' },
  },
  'type-class': {
    rung: 'kinds',
    trigger: 'Item types: the editor shows the type name and its standard class.',
    standard: 'rdfClasses',
    text: { title: 'typeClass.title', body: 'typeClass.body' },
  },
  'type-pick': {
    rung: 'kinds',
    trigger: 'Item form: the Type picker.',
    standard: 'rdfClasses',
    text: { title: 'typePick.title', body: 'typePick.body' },
  },
  standards: {
    rung: 'linkedData',
    trigger: 'Standards: the first visit.',
    standard: 'linkedData',
    text: { title: 'standards.title', body: 'standards.body' },
  },
  'local-field': {
    rung: 'makingStandards',
    trigger: 'Item types and forms: a field from the local vocabulary, as after "Create a new field".',
    standard: 'localVocabulary',
    text: { title: 'localField.title', body: 'localField.body' },
  },
  'term-merge': {
    rung: 'fixedValues',
    trigger: 'Term lists and the Tags panel: the first merge prompt for alike spellings.',
    standard: 'skos',
    text: { title: 'termMerge.title', body: 'termMerge.body' },
  },
  'tag-promote': {
    rung: 'fixedValues',
    trigger: 'Tags panel: the first "Promote to list" preview.',
    standard: 'skos',
    text: { title: 'tagPromote.title', body: 'tagPromote.body' },
  },
  'authority-link': {
    rung: 'linkedData',
    trigger: "Publish panel of a term list: the first time a term's Links opens.",
    standard: 'skosMapping',
    text: { title: 'authorityLink.title', body: 'authorityLink.body' },
  },
  publish: {
    rung: 'makingStandards',
    trigger: 'Publish panel: a term list or vocabulary that was never published.',
    standard: 'coolUris',
    text: { title: 'publish.title', body: 'publish.body' },
  },
} as const satisfies Readonly<Record<string, Lesson>>;

export type LessonId = keyof typeof LESSONS;

/** The rung's place on the ladder, from 1. */
export const rungNumber = (rung: Rung): number => RUNGS.indexOf(rung) + 1;
