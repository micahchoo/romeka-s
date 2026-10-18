/**
 * The edit-time model behind every write form. A `ValueDraft` is one row of the
 * values editor; `toValueWrites` turns a validated draft into the wire shape
 * `ValueHydrator` reads. Values with no editable representation are dropped
 * here, before they can reach a request body.
 */
import { resourceIdFromParam } from '../api/parse';
import type { Translator } from '../i18n';
import type {
  AssetId,
  ClassId,
  PropertyId,
  Property,
  ResourceEntity,
  TemplateId,
  UserId,
  Value,
  ValueWrite,
} from '../api/types';

export type ValueKind = 'literal' | 'uri' | 'resource';

export interface ValueDraft {
  /** Stable React key for one row. */
  readonly key: string;
  readonly propertyId: PropertyId;
  readonly term: string;
  readonly kind: ValueKind;
  /** Literal text, URI, or the resource id as typed. */
  readonly text: string;
  /** `o:label` of a URI value. */
  readonly label: string;
  readonly lang: string;
  readonly isPublic: boolean;
}

export interface ResourceDraft {
  readonly isPublic: boolean;
  readonly owner: UserId | null;
  readonly resourceClass: ClassId | null;
  readonly resourceTemplate: TemplateId | null;
  readonly thumbnail: AssetId | null;
  readonly values: readonly ValueDraft[];
}

let nextKey = 0;

const draftKey = (): string => {
  nextKey += 1;
  return `value-${String(nextKey)}`;
};

const textOf = (value: Value): string => {
  switch (value.kind) {
    case 'literal':
      return value.text;
    case 'uri':
      return value.uri;
    case 'resource':
      return String(value.resourceId);
    case 'other':
      return '';
  }
};

const kindOf = (value: Value): ValueKind | null => {
  switch (value.kind) {
    case 'literal':
      return 'literal';
    case 'uri':
      return 'uri';
    case 'resource':
      return 'resource';
    case 'other':
      return null;
  }
};

const draftFromValue = (term: string, value: Value): ValueDraft | null => {
  const kind = kindOf(value);
  if (kind === null) {
    return null;
  }
  return {
    key: draftKey(),
    propertyId: value.propertyId,
    term,
    kind,
    text: textOf(value),
    label: value.kind === 'uri' ? (value.label ?? '') : '',
    lang: value.kind === 'literal' ? (value.lang ?? '') : '',
    isPublic: value.isPublic,
  };
};

export const emptyResourceDraft = (): ResourceDraft => ({
  isPublic: true,
  owner: null,
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  values: [],
});

/** The draft a stored entity edits into, dropping values the base cannot render. */
export const resourceDraftFrom = (entity: ResourceEntity<number>): ResourceDraft => ({
  isPublic: entity.isPublic,
  owner: entity.owner?.id ?? null,
  resourceClass: entity.resourceClass?.id ?? null,
  resourceTemplate: entity.resourceTemplate?.id ?? null,
  thumbnail: entity.thumbnail?.id ?? null,
  values: [...entity.values].flatMap(([term, entries]) =>
    entries.flatMap((value) => {
      const draft = draftFromValue(term, value);
      return draft === null ? [] : [draft];
    }),
  ),
});

const emptyToNull = (text: string): string | null => {
  const trimmed = text.trim();
  return trimmed === '' ? null : trimmed;
};

/** Turns a validated draft into the value objects of a write body. */
export const toValueWrites = (values: readonly ValueDraft[]): readonly ValueWrite[] =>
  values.flatMap((value): ValueWrite[] => {
    switch (value.kind) {
      case 'literal':
        return [
          {
            term: value.term,
            propertyId: value.propertyId,
            type: 'literal',
            text: value.text,
            lang: emptyToNull(value.lang),
            isPublic: value.isPublic,
          },
        ];
      case 'uri':
        return [
          {
            term: value.term,
            propertyId: value.propertyId,
            type: 'uri',
            uri: emptyToNull(value.text) ?? '',
            label: emptyToNull(value.label),
            isPublic: value.isPublic,
          },
        ];
      case 'resource': {
        const resourceId = resourceIdFromParam(value.text);
        return resourceId === null
          ? []
          : [
              {
                term: value.term,
                propertyId: value.propertyId,
                type: 'resource',
                resourceId,
                isPublic: value.isPublic,
              },
            ];
      }
    }
  });

/** Every reason the draft cannot be sent, in the API's own terms. */
export const valueErrors = (
  values: readonly ValueDraft[],
  t: Translator['t'],
): readonly string[] => {
  const errors: string[] = [];
  for (const value of values) {
    if (value.text.trim() !== '') {
      continue;
    }
    if (value.kind === 'literal') {
      errors.push(t('draft.needsValue', { term: value.term }));
    } else if (value.kind === 'uri') {
      errors.push(t('draft.needsUri', { term: value.term }));
    }
  }
  for (const value of values) {
    if (value.kind === 'resource' && resourceIdFromParam(value.text) === null) {
      errors.push(t('draft.needsResourceId', { term: value.term }));
    }
  }
  return errors;
};

export const addValue = (draft: ResourceDraft, property: Property): ResourceDraft => ({
  ...draft,
  values: [
    ...draft.values,
    {
      key: draftKey(),
      propertyId: property.id,
      term: property.term,
      kind: 'literal',
      text: '',
      label: '',
      lang: '',
      isPublic: true,
    },
  ],
});

export const removeValue = (draft: ResourceDraft, key: string): ResourceDraft => ({
  ...draft,
  values: draft.values.filter((value) => value.key !== key),
});

export const patchValue = (
  draft: ResourceDraft,
  key: string,
  patch: Partial<ValueDraft>,
): ResourceDraft => ({
  ...draft,
  values: draft.values.map((value) => (value.key === key ? { ...value, ...patch } : value)),
});

/** The text of the first literal value for a term, or the empty string. */
export const literalTextOf = (values: readonly ValueDraft[], term: string): string => {
  const value = values.find((entry) => entry.term === term && entry.kind === 'literal');
  return value?.text ?? '';
};

/**
 * Replaces the literal values of one term with a single one. An empty text
 * removes every value of the term, which is what clearing a title means.
 */
export const withLiteralText = (
  draft: ResourceDraft,
  property: Property,
  text: string,
): ResourceDraft => {
  const others = draft.values.filter((value) => value.term !== property.term);
  if (text.trim() === '') {
    return { ...draft, values: others };
  }
  const existing = draft.values.find(
    (value) => value.term === property.term && value.kind === 'literal',
  );
  return {
    ...draft,
    values: [
      ...others,
      {
        key: existing?.key ?? draftKey(),
        propertyId: property.id,
        term: property.term,
        kind: 'literal',
        text,
        label: '',
        lang: existing?.lang ?? '',
        isPublic: existing?.isPublic ?? true,
      },
    ],
  };
};
