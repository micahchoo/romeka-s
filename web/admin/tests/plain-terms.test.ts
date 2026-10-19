/**
 * The plain-words search: a description finds the standard term, ranked, and
 * a loose query does not return most of the vocabulary.
 */
import { describe, expect, it } from 'vitest';
import { plainName, searchTerms, type Termed } from '../src/app/plain-terms';
import { localNameFor } from '../src/app/local-fields';

const entry = (term: string, label: string, comment: string | null = null): Termed => ({
  term,
  localName: term.split(':')[1] ?? term,
  label,
  comment,
});

/** A slice of the default install's properties, with their stored comments. */
const PROPERTIES: readonly Termed[] = [
  entry('dcterms:title', 'Title', 'A name given to the resource.'),
  entry('dcterms:creator', 'Creator', 'An entity responsible for making the resource.'),
  entry(
    'dcterms:contributor',
    'Contributor',
    'An entity responsible for making contributions to the resource.',
  ),
  entry(
    'dcterms:publisher',
    'Publisher',
    'An entity responsible for making the resource available.',
  ),
  entry(
    'dcterms:date',
    'Date',
    'A point or period of time associated with an event in the lifecycle of the resource.',
  ),
  entry('dcterms:created', 'Date Created', 'Date of creation of the resource.'),
  entry('dcterms:extent', 'Extent', 'The size or duration of the resource.'),
  entry('dcterms:subject', 'Subject', 'The topic of the resource.'),
  entry('dcterms:rights', 'Rights', 'Information about rights held in and over the resource.'),
  entry(
    'dcterms:provenance',
    'Provenance',
    'A statement of any changes in ownership and custody of the resource.',
  ),
  entry('bibo:numPages', 'number of pages', 'The number of pages contained in a document'),
  entry('bibo:pages', 'pages', 'A string of non-contiguous page spans'),
  entry('bibo:recipient', 'recipient', 'An agent that receives a communication document.'),
  entry('foaf:name', 'name', 'A name for some thing.'),
  entry('local:condition', 'Condition', null),
];

const terms = (query: string, locale: 'en' | 'es' = 'en') =>
  searchTerms(PROPERTIES, query, locale).map((found) => found.term);

describe('plain-words search', () => {
  it('finds the creator from how a person asks for it', () => {
    expect(terms('who wrote it')[0]).toBe('dcterms:creator');
    expect(terms('photographer')).toEqual(['dcterms:creator']);
  });

  it('ranks the page count before the size and cuts what only shares a weak word', () => {
    expect(terms('how many pages')).toEqual(['bibo:numPages', 'dcterms:extent', 'bibo:pages']);
  });

  it('matches a term or a label exactly before anything else', () => {
    expect(terms('dcterms:date')[0]).toBe('dcterms:date');
    expect(terms('Title')[0]).toBe('dcterms:title');
  });

  it('finds a term the table does not know from its label', () => {
    expect(terms('condition')).toEqual(['local:condition']);
  });

  it('reads Spanish words in the Spanish locale', () => {
    expect(terms('quién escribió', 'es')[0]).toBe('dcterms:creator');
  });

  it('finds nothing for an empty or unrelated query', () => {
    expect(terms('   ')).toEqual([]);
    expect(terms('zebra')).toEqual([]);
  });

  it('names a common term in plain words and any other by its label', () => {
    expect(plainName(entry('dcterms:provenance', 'Provenance'), 'en')).toBe('Where it came from');
    expect(plainName(entry('dcterms:provenance', 'Provenance'), 'es')).toBe('Procedencia');
    expect(plainName(entry('local:condition', 'Condition'), 'en')).toBe('Condition');
  });
});

describe('a new local field', () => {
  it('takes a camel-case local name from its plain name', () => {
    expect(localNameFor('Number of pages')).toBe('numberOfPages');
    expect(localNameFor('Estado de conservación')).toBe('estadoDeConservacion');
    expect(localNameFor('1st owner')).toBe('field1stOwner');
  });
});
