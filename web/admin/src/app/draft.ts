/**
 * The edit-time model behind every write form. A `ValueDraft` is one row of the
 * values editor; `toValueWrites` turns a validated draft into the wire shape
 * `ValueHydrator` reads. Values with no editable representation are dropped
 * here, before they can reach a request body.
 */
import { resourceIdFromParam } from '../api/parse';
import { TAG_TERM, TAG_TYPE } from '../api/tags';
import type { KeyOf, MessageKey, Translator } from '../i18n';
import type { common } from '../i18n/namespaces/common';
import type {
  AssetId,
  ClassId,
  PropertyId,
  ResourceEntity,
  TemplateId,
  UserId,
  Value,
  ValueWrite,
} from '../api/types';

/** The editable shape of a value: which members its data type reads. */
export type ValueKind = 'literal' | 'uri' | 'resource';

export interface ValueDraft {
  /** Stable React key for one row. */
  readonly key: string;
  readonly propertyId: PropertyId;
  readonly term: string;
  /** The data type name the write sends: `literal`, `resource:item`, a module's type. */
  readonly type: string;
  readonly kind: ValueKind;
  /** Literal text, URI, or the resource id as typed. */
  readonly text: string;
  /** `o:label` of a URI value. */
  readonly label: string;
  /** `@language` of a literal, `o:lang` of a URI. */
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
  /**
   * The tags: plain-text `dcterms:subject` values, edited in the Tags field
   * and kept out of `values` so the values editor never shows them twice.
   */
  readonly tags: readonly ValueDraft[];
}

let nextKey = 0;

export const draftKey = (): string => {
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

const langOf = (value: Value): string => {
  switch (value.kind) {
    case 'literal':
    case 'uri':
      return value.lang ?? '';
    case 'resource':
    case 'other':
      return '';
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
    type: value.type,
    kind,
    text: textOf(value),
    label: value.kind === 'uri' ? (value.label ?? '') : '',
    lang: langOf(value),
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
  tags: [],
});

/** A tag: a `dcterms:subject` value typed `literal`. A URI or list value there is not one. */
export const isTag = (value: Pick<ValueDraft, 'term' | 'type'>): boolean =>
  value.term === TAG_TERM && value.type === TAG_TYPE;

/** The draft a stored entity edits into, dropping values the base cannot render. */
export const resourceDraftFrom = (entity: ResourceEntity<number>): ResourceDraft => {
  const drafts = [...entity.values].flatMap(([term, entries]) =>
    entries.flatMap((value) => {
      const draft = draftFromValue(term, value);
      return draft === null ? [] : [draft];
    }),
  );
  return {
    isPublic: entity.isPublic,
    owner: entity.owner?.id ?? null,
    resourceClass: entity.resourceClass?.id ?? null,
    resourceTemplate: entity.resourceTemplate?.id ?? null,
    thumbnail: entity.thumbnail?.id ?? null,
    values: drafts.filter((draft) => !isTag(draft)),
    tags: drafts.filter(isTag),
  };
};

/** Every value the draft writes: the values editor's, then the tags. */
export const allValues = (draft: ResourceDraft): readonly ValueDraft[] => [
  ...draft.values,
  ...draft.tags,
];

const emptyToNull = (text: string): string | null => {
  const trimmed = text.trim();
  return trimmed === '' ? null : trimmed;
};

/**
 * A value with nothing typed in its main input. `ValueHydrator` skips it as an
 * invalid value, so the form leaves it out of the write rather than sending it.
 */
export const isBlank = (value: ValueDraft): boolean => value.text.trim() === '';

/** Turns a validated draft into the value objects of a write body. */
export const toValueWrites = (values: readonly ValueDraft[]): readonly ValueWrite[] =>
  values.flatMap((value): ValueWrite[] => {
    if (isBlank(value)) {
      return [];
    }
    const common = { term: value.term, propertyId: value.propertyId, type: value.type };
    switch (value.kind) {
      case 'literal':
        return [
          {
            ...common,
            kind: 'literal',
            text: value.text,
            lang: emptyToNull(value.lang),
            isPublic: value.isPublic,
          },
        ];
      case 'uri':
        return [
          {
            ...common,
            kind: 'uri',
            uri: value.text.trim(),
            label: emptyToNull(value.label),
            lang: emptyToNull(value.lang),
            isPublic: value.isPublic,
          },
        ];
      case 'resource': {
        const resourceId = resourceIdFromParam(value.text);
        return resourceId === null
          ? []
          : [{ ...common, kind: 'resource', resourceId, isPublic: value.isPublic }];
      }
    }
  });

/** Every reason the draft cannot be sent, in the API's own terms. */
export const valueErrors = (
  values: readonly ValueDraft[],
  t: Translator<MessageKey | KeyOf<typeof common>>['t'],
): readonly string[] =>
  values
    .filter(
      (value) =>
        value.kind === 'resource' && !isBlank(value) && resourceIdFromParam(value.text) === null,
    )
    .map((value) => t('draft.needsResourceId', { term: value.term }));

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
