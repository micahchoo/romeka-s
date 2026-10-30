/**
 * The fields the Items sort list offers.
 */
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import type { PropertyId, TemplateId } from '../src/api/types';
import { NO_OPTIONS } from '../src/pages/items/browse-options';
import type { BrowseOptions } from '../src/pages/items/browse-options';
import { sortFields } from '../src/pages/items/sort-fields';
import type { TermLabels } from '../src/i18n/termText';

const property = (id: number): PropertyId => brandId<'PropertyId'>(id);
const template = (id: number): TemplateId => brandId<'TemplateId'>(id);

const IDENTIFIER = property(10);
const DATE = property(7);
const BOX = property(30);
const ARTIFACT = template(1);
const LETTER = template(2);

const text: TermLabels = { locale: 'en', label: (_term, label) => label };

const options = (fields: ReadonlyMap<TemplateId, readonly PropertyId[]>): BrowseOptions => ({
  ...NO_OPTIONS,
  properties: new Map([
    [IDENTIFIER, { term: 'dcterms:identifier', label: 'Identifier' }],
    [DATE, { term: 'dcterms:date', label: 'Date' }],
    [BOX, { term: 'bibo:locator', label: 'Locator' }],
  ]),
  templateFields: new Map(
    [...fields].map(([id, properties]) => [
      id,
      properties.map((entry) => ({ property: entry, alternateLabel: entry === BOX ? 'Box' : null })),
    ]),
  ),
});

const terms = (fields: readonly { readonly term: string }[]): readonly string[] => fields.map((field) => field.term);

describe('the Items sort fields', () => {
  it('offer Identifier when any item type has it, and no other field until a type is chosen', () => {
    const withIdentifier = options(new Map([[ARTIFACT, [DATE, IDENTIFIER]], [LETTER, [DATE]]]));
    expect(sortFields(withIdentifier, { template: null, chosen: 'newest', setHasIdentifier: false }, text)).toEqual([
      { term: 'dcterms:identifier', name: 'Reference number' },
    ]);
    const without = options(new Map([[LETTER, [DATE]]]));
    expect(sortFields(without, { template: null, chosen: 'newest', setHasIdentifier: false }, text)).toEqual([]);
  });

  it('offer Identifier first, then the chosen type’s own fields by the names the type gives them', () => {
    const shelf = options(new Map([[ARTIFACT, [DATE, BOX, IDENTIFIER]]]));
    expect(sortFields(shelf, { template: ARTIFACT, chosen: 'newest', setHasIdentifier: false }, text)).toEqual([
      { term: 'dcterms:identifier', name: 'Reference number' },
      { term: 'bibo:locator', name: 'Box' },
      { term: 'dcterms:date', name: 'Date' },
    ]);
  });

  it('offer Identifier when the set’s items have one, though no item type names it', () => {
    const none = options(new Map([[LETTER, [DATE]]]));
    expect(terms(sortFields(none, { template: null, chosen: 'newest', setHasIdentifier: true }, text))).toEqual([
      'dcterms:identifier',
    ]);
  });

  it('keep the chosen field, so the control always shows the order in use', () => {
    const none = options(new Map());
    expect(sortFields(none, { template: null, chosen: 'dcterms:date', setHasIdentifier: false }, text)).toEqual([
      { term: 'dcterms:date', name: 'Date' },
    ]);
    expect(sortFields(NO_OPTIONS, { template: null, chosen: 'foaf:name', setHasIdentifier: false }, text)).toEqual([
      { term: 'foaf:name', name: 'foaf:name' },
    ]);
  });
});
