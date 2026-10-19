/**
 * The template-to-fields layout of the value editor, frozen from upstream
 * `application/asset/js/resource-form.js` (`applyResourceTemplate`,
 * `rewritePropertyField`, `makeNewValue`, the required check on submit).
 */
import { describe, expect, it } from 'vitest';
import { parseItem, parseProperty, parseResourceTemplate } from '../src/api/parse';
import type { Property, ResourceTemplate } from '../src/api/types';
import {
  emptyResourceDraft,
  patchValue,
  resourceDraftFrom,
  toValueWrites,
  type ResourceDraft,
} from '../src/app/draft';
import {
  addFieldValue,
  layoutFields,
  missingRequired,
  newResourceDraft,
  pickTerm,
  setValueType,
  typeChoices,
  valueInput,
  withTemplate,
  withTemplateRows,
  type PropertyField,
} from '../src/app/value-fields';
import { searchTerms } from '../src/app/plain-terms';

const property = (id: number, term: string, label: string, comment: string | null = null): Property =>
  parseProperty({
    '@id': `http://localhost/api/properties/${String(id)}`,
    '@type': 'o:Property',
    'o:id': id,
    'o:local_name': term.split(':')[1],
    'o:label': label,
    'o:comment': comment,
    'o:term': term,
    'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
  });

const PROPERTIES: readonly Property[] = [
  property(1, 'dcterms:title', 'Title', 'A name given to the resource.'),
  property(2, 'dcterms:creator', 'Creator'),
  property(4, 'dcterms:description', 'Description'),
  property(8, 'dcterms:date', 'Date'),
  property(12, 'dcterms:relation', 'Relation'),
  property(40, 'bibo:annotates', 'annotates'),
];

interface Row {
  readonly id: number;
  readonly label?: string;
  readonly comment?: string;
  readonly dataTypes?: readonly string[];
  readonly required?: boolean;
  readonly private?: boolean;
  readonly lang?: string;
}

const template = (id: number, rows: readonly Row[], classId: number | null = null): ResourceTemplate =>
  parseResourceTemplate({
    '@id': `http://localhost/api/resource_templates/${String(id)}`,
    '@type': 'o:ResourceTemplate',
    'o:id': id,
    'o:label': `Template ${String(id)}`,
    'o:owner': null,
    'o:resource_class':
      classId === null ? null : { '@id': `http://localhost/api/resource_classes/${String(classId)}`, 'o:id': classId },
    'o:title_property': null,
    'o:description_property': null,
    'o:resource_template_property': rows.map((row) => ({
      'o:property': { '@id': `http://localhost/api/properties/${String(row.id)}`, 'o:id': row.id },
      'o:alternate_label': row.label ?? null,
      'o:alternate_comment': row.comment ?? null,
      'o:data_type': row.dataTypes ?? [],
      'o:is_required': row.required ?? false,
      'o:is_private': row.private ?? false,
      'o:default_lang': row.lang ?? null,
    })),
  });

/** A stored item with a creator, a date and a relation, and no template. */
const storedDraft = (): ResourceDraft =>
  resourceDraftFrom(
    parseItem({
      '@id': 'http://localhost/api/items/5',
      '@type': 'o:Item',
      'o:id': 5,
      'o:is_public': true,
      'o:owner': null,
      'o:resource_class': null,
      'o:resource_template': null,
      'o:thumbnail': null,
      'o:title': null,
      'thumbnail_display_urls': {},
      'o:created': { '@value': '2026-10-18T00:00:00+00:00', '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
      'o:modified': null,
      'o:primary_media': null,
      'o:media': [],
      'o:item_set': [],
      'o:site': [],
      'dcterms:date': [
        { type: 'literal', property_id: 8, property_label: 'Date', is_public: true, '@value': '1901' },
      ],
      'dcterms:creator': [
        { type: 'literal', property_id: 2, property_label: 'Creator', is_public: true, '@value': 'Ada' },
      ],
      'dcterms:relation': [
        {
          type: 'resource:item',
          property_id: 12,
          property_label: 'Relation',
          is_public: true,
          '@id': 'http://localhost/api/items/9',
          value_resource_id: 9,
          value_resource_name: 'items',
        },
      ],
    }),
  );

const terms = (fields: readonly PropertyField[]): readonly string[] => fields.map((field) => field.term);

const texts = (draft: ResourceDraft, term: string): readonly string[] =>
  draft.values.filter((value) => value.term === term).map((value) => value.text);

const fieldOf = (fields: readonly PropertyField[], term: string): PropertyField => {
  const found = fields.find((field) => field.term === term);
  if (found === undefined) {
    throw new Error(`no field for ${term}`);
  }
  return found;
};

describe('the template lays out the value fields', () => {
  const portrait = template(7, [
    { id: 4, label: 'Caption', comment: 'What the picture shows.' },
    { id: 2, label: 'Photographer', required: true },
    { id: 1 },
  ]);

  it('puts the template rows first, in template order, then the other properties as they appear', () => {
    const fields = layoutFields(storedDraft().values, portrait, PROPERTIES);
    expect(terms(fields)).toEqual([
      'dcterms:description',
      'dcterms:creator',
      'dcterms:title',
      'dcterms:date',
      'dcterms:relation',
    ]);
  });

  it('labels a template row with its alternate label and comment, and keeps the term beside it', () => {
    const fields = layoutFields(storedDraft().values, portrait, PROPERTIES);
    expect(fieldOf(fields, 'dcterms:description')).toMatchObject({
      label: 'Caption',
      comment: 'What the picture shows.',
      term: 'dcterms:description',
    });
    expect(fieldOf(fields, 'dcterms:title')).toMatchObject({
      label: 'Title',
      comment: 'A name given to the resource.',
    });
    expect(fieldOf(fields, 'dcterms:date')).toMatchObject({ label: 'Date', rule: null });
  });

  it('gives a template row the stored values of its property', () => {
    const fields = layoutFields(storedDraft().values, portrait, PROPERTIES);
    expect(fieldOf(fields, 'dcterms:creator').values.map((value) => value.text)).toEqual(['Ada']);
    expect(fieldOf(fields, 'dcterms:description').values).toEqual([]);
  });

  it('leaves out a template row whose property the property list does not name', () => {
    const fields = layoutFields([], template(8, [{ id: 999 }, { id: 2 }]), PROPERTIES);
    expect(terms(fields)).toEqual(['dcterms:creator']);
  });

  it('with no template, lists the properties in the order their values appear', () => {
    expect(terms(layoutFields(storedDraft().values, null, PROPERTIES))).toEqual([
      'dcterms:date',
      'dcterms:creator',
      'dcterms:relation',
    ]);
  });
});

describe('choosing a template', () => {
  const strict = template(
    9,
    [
      { id: 2, required: true, private: true, lang: 'fr', dataTypes: ['uri', 'literal'] },
      { id: 4 },
    ],
    13,
  );

  it('adds one empty value to each template row that has none, with the row defaults', () => {
    const draft = withTemplate(emptyResourceDraft(), strict, PROPERTIES);
    expect(draft.resourceTemplate).toBe(9);
    expect(draft.values.map((value) => [value.term, value.type, value.lang, value.isPublic])).toEqual([
      ['dcterms:creator', 'uri', 'fr', false],
      ['dcterms:description', 'literal', '', true],
    ]);
  });

  it('sets the type class unless the user chose another class', () => {
    expect(withTemplate(emptyResourceDraft(), strict, PROPERTIES).resourceClass).toBe(13);
    const classed = { ...emptyResourceDraft(), resourceClass: brandedClass(40) };
    expect(withTemplate(classed, strict, PROPERTIES).resourceClass).toBe(40);
  });

  it('moves the class to the next type when it was the previous type\'s class', () => {
    const first = withTemplate(emptyResourceDraft(), strict, PROPERTIES);
    const next = template(10, [{ id: 8 }], 51);
    expect(withTemplate(first, next, PROPERTIES, strict).resourceClass).toBe(51);
    const overridden = { ...first, resourceClass: brandedClass(40) };
    expect(withTemplate(overridden, next, PROPERTIES, strict).resourceClass).toBe(40);
  });

  it('never drops a typed value, in or out of the template, and switching back to no template keeps everything', () => {
    const chosen = withTemplate(storedDraft(), strict, PROPERTIES);
    expect(texts(chosen, 'dcterms:creator')).toEqual(['Ada']);
    expect(texts(chosen, 'dcterms:date')).toEqual(['1901']);
    expect(texts(chosen, 'dcterms:relation')).toEqual(['9']);
    const none = withTemplate(chosen, null, PROPERTIES);
    expect(none.resourceTemplate).toBeNull();
    expect(none.values).toEqual(chosen.values);
  });

  it('replaces every empty value with the next template rows and their defaults', () => {
    const first = withTemplate(emptyResourceDraft(), strict, PROPERTIES);
    const typed = patchValue(first, valueKey(first, 'dcterms:creator'), { text: 'http://example.org/ada' });
    const second = withTemplate(typed, template(10, [{ id: 8 }]), PROPERTIES);
    expect(second.values.map((value) => [value.term, value.text])).toEqual([
      ['dcterms:creator', 'http://example.org/ada'],
      ['dcterms:date', ''],
    ]);
  });

  it('lays out the rows of a stored resource the same way when its form opens', () => {
    const stored = { ...storedDraft(), resourceTemplate: strict.id };
    const opened = withTemplateRows(stored, strict, PROPERTIES);
    expect(opened.resourceClass).toBeNull();
    expect(texts(opened, 'dcterms:creator')).toEqual(['Ada']);
    expect(texts(opened, 'dcterms:description')).toEqual(['']);
  });
});

describe('a new resource', () => {
  it('starts with an empty title and description, as the upstream add form does', () => {
    const draft = newResourceDraft(PROPERTIES);
    expect(draft.values.map((value) => [value.term, value.text])).toEqual([
      ['dcterms:title', ''],
      ['dcterms:description', ''],
    ]);
  });
});

describe('required template rows', () => {
  const required = template(11, [{ id: 2, label: 'Photographer', required: true }, { id: 1 }]);

  it('names a required row that has no filled value', () => {
    const draft = withTemplate(emptyResourceDraft(), required, PROPERTIES);
    const blank = patchValue(draft, valueKey(draft, 'dcterms:creator'), { text: '   ' });
    expect(missingRequired(layoutFields(blank.values, required, PROPERTIES)).map((f) => f.label)).toEqual([
      'Photographer',
    ]);
  });

  it('is satisfied by one filled value', () => {
    const draft = withTemplate(emptyResourceDraft(), required, PROPERTIES);
    const filled = patchValue(draft, valueKey(draft, 'dcterms:creator'), { text: 'Ada' });
    expect(missingRequired(layoutFields(filled.values, required, PROPERTIES))).toEqual([]);
  });
});

describe('data types on a row', () => {
  it('offers only the data types the template row lists', () => {
    const rowTemplate = template(12, [{ id: 12, dataTypes: ['resource:item', 'uri'] }]);
    const draft = withTemplate(emptyResourceDraft(), rowTemplate, PROPERTIES);
    const field = fieldOf(layoutFields(draft.values, rowTemplate, PROPERTIES), 'dcterms:relation');
    const [value] = field.values;
    if (value === undefined) {
      throw new Error('expected the seeded relation value');
    }
    expect(typeChoices(field, value)).toEqual(['resource:item', 'uri']);
    expect(value.kind).toBe('resource');
  });

  it('offers the three core kinds when the row lists none, and keeps a stored type it does not list', () => {
    const field = fieldOf(layoutFields(storedDraft().values, null, PROPERTIES), 'dcterms:relation');
    const [value] = field.values;
    if (value === undefined) {
      throw new Error('expected the stored relation value');
    }
    expect(typeChoices(field, value)).toEqual(['literal', 'uri', 'resource', 'resource:item']);
  });

  it('edits a data type it does not know as its base kind', () => {
    const draft = storedDraft();
    const key = valueKey(draft, 'dcterms:date');
    const changed = setValueType(draft, key, 'customvocab:3');
    expect(changed.values.find((value) => value.key === key)).toMatchObject({
      type: 'customvocab:3',
      kind: 'literal',
      text: '1901',
    });
    expect(setValueType(draft, key, 'uri').values.find((value) => value.key === key)?.kind).toBe('uri');
  });

  it('adds a value with the row defaults', () => {
    const rowTemplate = template(13, [{ id: 2, private: true, lang: 'en', dataTypes: ['literal'] }]);
    const draft = withTemplate(emptyResourceDraft(), rowTemplate, PROPERTIES);
    const field = fieldOf(layoutFields(draft.values, rowTemplate, PROPERTIES), 'dcterms:creator');
    const added = addFieldValue(draft, field);
    expect(added.values.map((value) => [value.term, value.lang, value.isPublic])).toEqual([
      ['dcterms:creator', 'en', false],
      ['dcterms:creator', 'en', false],
    ]);
  });
});

describe('a term list on a row (Custom Vocab)', () => {
  const INSTALLED = [
    { id: 'customvocab:3', label: 'Places' },
    { id: 'customvocab:5', label: 'Links' },
  ];
  const only = (field: PropertyField): PropertyField['values'][number] => {
    const [value] = field.values;
    if (value === undefined) {
      throw new Error(`expected one value on ${field.term}`);
    }
    return value;
  };

  it('seeds a row whose first type is a term list with a value edited through that list', () => {
    const rowTemplate = template(20, [{ id: 2, dataTypes: ['customvocab:3', 'literal'] }]);
    const draft = withTemplate(emptyResourceDraft(), rowTemplate, PROPERTIES);
    const field = fieldOf(layoutFields(draft.values, rowTemplate, PROPERTIES), 'dcterms:creator');
    const value = only(field);
    expect(value.type).toBe('customvocab:3');
    expect(typeChoices(field, value)).toEqual(['customvocab:3', 'literal']);
    expect(valueInput(value, INSTALLED)).toEqual({ kind: 'term-list', termList: 'customvocab:3' });
  });

  it('offers the list as a kind beside text, and switching to it edits through the list', () => {
    const rowTemplate = template(21, [{ id: 2, dataTypes: ['literal', 'customvocab:3'] }]);
    const draft = withTemplate(emptyResourceDraft(), rowTemplate, PROPERTIES);
    const field = fieldOf(layoutFields(draft.values, rowTemplate, PROPERTIES), 'dcterms:creator');
    const value = only(field);
    expect(typeChoices(field, value)).toEqual(['literal', 'customvocab:3']);
    expect(valueInput(value, INSTALLED)).toEqual({ kind: 'literal' });
    const switched = setValueType(draft, value.key, 'customvocab:3');
    expect(valueInput(only(fieldOf(layoutFields(switched.values, rowTemplate, PROPERTIES), 'dcterms:creator')), INSTALLED)).toEqual({
      kind: 'term-list',
      termList: 'customvocab:3',
    });
  });

  it('edits a value of a list the server no longer lists as its base kind', () => {
    const rowTemplate = template(23, [{ id: 2, dataTypes: ['customvocab:999'] }]);
    const draft = withTemplate(emptyResourceDraft(), rowTemplate, PROPERTIES);
    const value = only(fieldOf(layoutFields(draft.values, rowTemplate, PROPERTIES), 'dcterms:creator'));
    expect(value.type).toBe('customvocab:999');
    expect(valueInput(value, INSTALLED)).toEqual({ kind: 'literal' });
  });

  it('stores a picked entry in its own shape and sends the list type back unchanged', () => {
    const rowTemplate = template(22, [{ id: 2, dataTypes: ['customvocab:5'] }]);
    const draft = withTemplate(emptyResourceDraft(), rowTemplate, PROPERTIES);
    const key = valueKey(draft, 'dcterms:creator');
    const common = { term: 'dcterms:creator', propertyId: 2, type: 'customvocab:5', isPublic: true };
    const uri = pickTerm(draft, key, { kind: 'uri', value: 'http://example.org/cork', label: 'Cork' });
    expect(toValueWrites(uri.values)).toEqual([
      { ...common, kind: 'uri', uri: 'http://example.org/cork', label: 'Cork', lang: null },
    ]);
    const bare = pickTerm(draft, key, { kind: 'uri', value: 'http://example.org/x', label: 'http://example.org/x' });
    expect(toValueWrites(bare.values)).toEqual([
      { ...common, kind: 'uri', uri: 'http://example.org/x', label: null, lang: null },
    ]);
    const item = pickTerm(draft, key, { kind: 'resource', value: '9', label: 'A letter' });
    expect(toValueWrites(item.values)).toEqual([{ ...common, kind: 'resource', resourceId: 9 }]);
    const word = pickTerm(item, key, { kind: 'literal', value: 'Cork', label: 'Cork' });
    expect(toValueWrites(word.values)).toEqual([{ ...common, kind: 'literal', text: 'Cork', lang: null }]);
    expect(toValueWrites(pickTerm(word, key, null).values)).toEqual([]);
  });

  it('opens a stored term list value as that list and saves it back unchanged', () => {
    const stored = resourceDraftFrom(
      parseItem({
        '@id': 'http://localhost/api/items/6',
        '@type': 'o:Item',
        'o:id': 6,
        'o:is_public': true,
        'o:owner': null,
        'o:resource_class': null,
        'o:resource_template': null,
        'o:thumbnail': null,
        'o:title': null,
        'thumbnail_display_urls': {},
        'o:created': { '@value': '2026-10-18T00:00:00+00:00', '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
        'o:modified': null,
        'o:primary_media': null,
        'o:media': [],
        'o:item_set': [],
        'o:site': [],
        'dcterms:creator': [
          { type: 'customvocab:3', property_id: 2, property_label: 'Creator', is_public: true, '@value': 'Cork', '@language': 'en' },
        ],
      }),
    );
    const [value] = stored.values;
    if (value === undefined) {
      throw new Error('expected the stored term list value');
    }
    expect(valueInput(value, INSTALLED)).toEqual({ kind: 'term-list', termList: 'customvocab:3' });
    expect(toValueWrites(stored.values)).toEqual([
      { term: 'dcterms:creator', propertyId: 2, type: 'customvocab:3', kind: 'literal', text: 'Cork', lang: 'en', isPublic: true },
    ]);
  });
});

describe('adding another field', () => {
  it('finds a property from plain words, its label or its term, best first', () => {
    const terms = (query: string) => searchTerms(PROPERTIES, query, 'en').map((entry) => entry.term);
    expect(terms('who wrote it')[0]).toBe('dcterms:creator');
    expect(terms('creat')).toEqual(['dcterms:creator']);
    expect(terms('BIBO:')).toEqual(['bibo:annotates']);
    expect(terms('Annot')).toEqual(['bibo:annotates']);
    expect(terms('  ')).toEqual([]);
    expect(terms('zebra')).toEqual([]);
  });
});

describe('the write a draft sends', () => {
  it('leaves out empty values and keeps each data type name and the uri language', () => {
    const draft = withTemplate(storedDraft(), template(14, [{ id: 4 }]), PROPERTIES);
    const uriKey = valueKey(draft, 'dcterms:creator');
    const withUri = patchValue(setValueType(draft, uriKey, 'uri'), uriKey, {
      text: 'http://example.org/ada',
      label: 'Ada',
      lang: 'en',
    });
    expect(toValueWrites(withUri.values)).toEqual([
      { term: 'dcterms:date', propertyId: 8, kind: 'literal', type: 'literal', text: '1901', lang: null, isPublic: true },
      {
        term: 'dcterms:creator',
        propertyId: 2,
        kind: 'uri',
        type: 'uri',
        uri: 'http://example.org/ada',
        label: 'Ada',
        lang: 'en',
        isPublic: true,
      },
      { term: 'dcterms:relation', propertyId: 12, kind: 'resource', type: 'resource:item', resourceId: 9, isPublic: true },
    ]);
  });
});

function valueKey(draft: ResourceDraft, term: string): string {
  const found = draft.values.find((value) => value.term === term);
  if (found === undefined) {
    throw new Error(`no value for ${term}`);
  }
  return found.key;
}

function brandedClass(id: number): ResourceDraft['resourceClass'] {
  const parsed = template(99, [], id).resourceClass;
  return parsed === null ? null : parsed.id;
}
