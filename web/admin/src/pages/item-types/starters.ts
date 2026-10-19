/**
 * "What kind of thing is this?": the starter kinds, as one table. A kind names
 * the standard class and the first fields of the type; the user then renames,
 * reorders or removes any of them. A kind the table does not know starts as a
 * physical object with a title, which fits anything one can hold.
 */
import type { Property, ResourceClass } from '../../api/types';
import type { Locale } from '../../i18n';
import { fieldKey, type FieldDraft, type TypeDraft } from './model';

type Localised = Readonly<Record<Locale, string>>;

interface StarterField {
  readonly term: string;
  readonly label: Localised;
  readonly dataTypes: readonly string[];
  readonly required?: true;
}

export interface Starter {
  readonly id: string;
  readonly name: Localised;
  /** What a user may type for this kind, in any shipped locale. */
  readonly words: readonly string[];
  readonly classTerm: string;
  /** The field the item's title comes from, when it is not `dcterms:title`. */
  readonly titleTerm?: string;
  readonly fields: readonly StarterField[];
}

const TEXT = ['literal'];
const ITEM = ['resource:item'];

const TITLE: StarterField = {
  term: 'dcterms:title',
  label: { en: 'Title', es: 'Título' },
  dataTypes: TEXT,
  required: true,
};

export const STARTERS: readonly Starter[] = [
  {
    id: 'letter',
    name: { en: 'Letter', es: 'Carta' },
    words: ['letter', 'letters', 'carta', 'cartas'],
    classTerm: 'bibo:Letter',
    fields: [
      TITLE,
      { term: 'dcterms:creator', label: { en: 'Sender', es: 'Remitente' }, dataTypes: TEXT },
      { term: 'bibo:recipient', label: { en: 'Recipient', es: 'Destinatario' }, dataTypes: TEXT },
      {
        term: 'dcterms:created',
        label: { en: 'Date written', es: 'Fecha de escritura' },
        dataTypes: TEXT,
      },
      { term: 'dcterms:spatial', label: { en: 'Place', es: 'Lugar' }, dataTypes: TEXT },
      {
        term: 'dcterms:description',
        label: { en: 'Description', es: 'Descripción' },
        dataTypes: TEXT,
      },
    ],
  },
  {
    id: 'photograph',
    name: { en: 'Photograph', es: 'Fotografía' },
    words: [
      'photograph',
      'photographs',
      'photo',
      'photos',
      'picture',
      'pictures',
      'fotografia',
      'fotografias',
      'foto',
      'fotos',
    ],
    classTerm: 'dctype:StillImage',
    fields: [
      TITLE,
      { term: 'dcterms:creator', label: { en: 'Photographer', es: 'Fotógrafo' }, dataTypes: TEXT },
      {
        term: 'dcterms:created',
        label: { en: 'Date taken', es: 'Fecha de la foto' },
        dataTypes: TEXT,
      },
      { term: 'dcterms:spatial', label: { en: 'Place', es: 'Lugar' }, dataTypes: TEXT },
      {
        term: 'dcterms:subject',
        label: { en: 'Who is in it', es: 'Quién aparece' },
        dataTypes: ITEM,
      },
    ],
  },
  {
    id: 'postcard',
    name: { en: 'Postcard', es: 'Postal' },
    words: ['postcard', 'postcards', 'postal', 'postales'],
    classTerm: 'bibo:Letter',
    fields: [
      TITLE,
      { term: 'dcterms:creator', label: { en: 'Sender', es: 'Remitente' }, dataTypes: TEXT },
      { term: 'bibo:recipient', label: { en: 'Recipient', es: 'Destinatario' }, dataTypes: TEXT },
      {
        term: 'dcterms:spatial',
        label: { en: 'Postmark place', es: 'Lugar del matasellos' },
        dataTypes: TEXT,
      },
    ],
  },
  {
    id: 'diary',
    name: { en: 'Diary', es: 'Diario' },
    words: ['diary', 'diaries', 'journal', 'journals', 'diario', 'diarios'],
    classTerm: 'bibo:Document',
    fields: [
      TITLE,
      { term: 'dcterms:creator', label: { en: 'Writer', es: 'Autor' }, dataTypes: TEXT },
      {
        term: 'dcterms:date',
        label: { en: 'Dates covered', es: 'Fechas que abarca' },
        dataTypes: TEXT,
      },
      {
        term: 'bibo:numPages',
        label: { en: 'Number of pages', es: 'Número de páginas' },
        dataTypes: TEXT,
      },
    ],
  },
  {
    id: 'person',
    name: { en: 'Person', es: 'Persona' },
    words: ['person', 'people', 'persons', 'persona', 'personas'],
    classTerm: 'foaf:Person',
    titleTerm: 'foaf:name',
    fields: [
      { term: 'foaf:name', label: { en: 'Name', es: 'Nombre' }, dataTypes: TEXT, required: true },
      {
        term: 'dcterms:description',
        label: { en: 'Relationship', es: 'Parentesco' },
        dataTypes: TEXT,
      },
      { term: 'dcterms:date', label: { en: 'Born', es: 'Nació' }, dataTypes: TEXT },
    ],
  },
];

/** What an unknown kind starts as. */
export const FALLBACK: Starter = {
  id: 'other',
  name: { en: 'Physical object', es: 'Objeto físico' },
  words: [],
  classTerm: 'dctype:PhysicalObject',
  fields: [TITLE],
};

const fold = (text: string): string =>
  text.trim().toLowerCase().normalize('NFD').replace(/\p{M}/gu, '');

/** The starter a typed kind names, or `null` for a kind the table does not know. */
export const starterFor = (kind: string): Starter | null => {
  const folded = fold(kind);
  return STARTERS.find((starter) => starter.words.includes(folded)) ?? null;
};

/**
 * A new type's draft: the kind's class and fields, those this installation
 * has. An alternate label is kept only where it differs from the property's
 * own label, so a "Title" row reads the property's label.
 */
export const draftFromStarter = (
  name: string,
  starter: Starter,
  locale: Locale,
  properties: readonly Property[],
  classes: readonly ResourceClass[],
): TypeDraft => {
  const byTerm = (term: string): Property | undefined =>
    properties.find((property) => property.term === term);
  const fields = starter.fields.flatMap((entry): FieldDraft[] => {
    const property = byTerm(entry.term);
    if (property === undefined) {
      return [];
    }
    const label = entry.label[locale];
    return [
      {
        key: fieldKey(),
        propertyId: property.id,
        alternateLabel: label === property.label ? null : label,
        alternateComment: null,
        dataTypes: entry.dataTypes,
        isRequired: entry.required === true,
        isPrivate: false,
        defaultLang: null,
      },
    ];
  });
  return {
    label: name.trim(),
    owner: null,
    resourceClass: classes.find((entry) => entry.term === starter.classTerm)?.id ?? null,
    titleProperty: starter.titleTerm === undefined ? null : (byTerm(starter.titleTerm)?.id ?? null),
    descriptionProperty: null,
    fields,
  };
};
