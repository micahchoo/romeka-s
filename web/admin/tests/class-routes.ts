/**
 * The class and standard lists a page reads to name classes. A class whose
 * label another class shares is named with its standard, so every page that
 * lists classes also lists the standards: spread `classRoutes` into a route
 * map. `classesWithTwin` and `vocabulariesWithSchema` add a second "Still
 * Image": `schema:StillImage` (60, Schema.org) beside the fixture's
 * `dctype:StillImage` (13, Dublin Core Type).
 */
import { isRecord, isUnknownArray } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import { jsonResponse } from './stub';
import type { Handler } from './stub';

/** The fixture classes and the fixture standards. */
export const classRoutes: Readonly<Record<string, Handler>> = {
  '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
  '/api/vocabularies': () => jsonResponse(loadFixture('vocabularies-page.json')),
};

const firstOf = (name: string): Record<string, unknown> => {
  const page = loadFixture(name);
  const entry = isUnknownArray(page) ? page[0] : undefined;
  if (!isRecord(entry)) {
    throw new Error(`expected a record first in ${name}`);
  }
  return entry;
};

const listOf = (name: string): readonly unknown[] => {
  const page = loadFixture(name);
  if (!isUnknownArray(page)) {
    throw new Error(`expected a list in ${name}`);
  }
  return page;
};

/** The fixture classes plus `schema:StillImage` (60). */
export const classesWithTwin = (): readonly unknown[] => [
  ...listOf('resource-classes-page.json'),
  {
    ...firstOf('resource-classes-page.json'),
    'o:id': 60,
    'o:term': 'schema:StillImage',
    'o:local_name': 'StillImage',
    'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/9', 'o:id': 9 },
  },
];

/** The fixture vocabularies plus Schema.org (9). */
export const vocabulariesWithSchema = (): readonly unknown[] => [
  ...listOf('vocabularies-page.json'),
  { ...firstOf('vocabularies-page.json'), 'o:id': 9, 'o:prefix': 'schema', 'o:label': 'Schema.org' },
];
