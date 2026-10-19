/**
 * Every lesson the admin can show, as data.
 *
 * A lesson teaches the standard behind one action, at the moment of that
 * action. The lessons climb one ladder (`RUNGS`): describe a thing, say what
 * kind it is, fix its values, link them, then make standards of your own.
 *
 * A lesson id is stored in each user's `romeka_lessons_seen` setting, so an id
 * never changes once it ships. A slot is a lesson whose screen another unit is
 * still building: it has an id and a rung, and no text, so
 * `useLesson` cannot render it (`WrittenLessonId` excludes it).
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
} as const;

export type StandardId = keyof typeof STANDARDS;

interface LessonText {
  readonly title: LessonsMessageKey;
  readonly body: LessonsMessageKey;
}

interface WrittenLesson {
  readonly rung: Rung;
  /** The UI moment that shows the lesson, for the next person who places one. */
  readonly trigger: string;
  readonly standard: StandardId;
  readonly text: LessonText;
}

/** A planned lesson: its screen is not built yet, so it names no standard or text. */
interface LessonSlot {
  readonly rung: Rung;
  readonly trigger: string;
  readonly standard: null;
  readonly text: null;
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
    trigger: 'Term lists: two terms merge into one (authority control).',
    standard: null,
    text: null,
  },
  'tag-promote': {
    rung: 'fixedValues',
    trigger: 'Tags: a free tag becomes a term in a vocabulary.',
    standard: null,
    text: null,
  },
  'authority-link': {
    rung: 'linkedData',
    trigger: 'Term lists: a term links to an authority record.',
    standard: null,
    text: null,
  },
  publish: {
    rung: 'makingStandards',
    trigger: 'Publishing: a record or vocabulary gets an address that does not change.',
    standard: null,
    text: null,
  },
} as const satisfies Readonly<Record<string, WrittenLesson | LessonSlot>>;

export type LessonId = keyof typeof LESSONS;

/** A lesson with text, the only kind a screen can show. */
export type WrittenLessonId = {
  [Id in LessonId]: (typeof LESSONS)[Id]['text'] extends null ? never : Id;
}[LessonId];

/** The rung's place on the ladder, from 1. */
export const rungNumber = (rung: Rung): number => RUNGS.indexOf(rung) + 1;
