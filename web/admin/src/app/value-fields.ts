/**
 * How a resource template drives the value editor, after upstream
 * `application/asset/js/resource-form.js`.
 *
 * The draft holds the values as one flat list, and the template never edits
 * that list except to add empty values. `layoutFields` derives the property
 * groups the form shows from the list and the template, so switching templates
 * cannot lose a typed value: the values stay, and only their grouping, labels
 * and rules change.
 */
import { isTermListName } from '../api/term-lists';
import type {
  DataType,
  Property,
  PropertyId,
  ResourceEntity,
  ResourceTemplate,
  TemplateId,
  TermListName,
} from '../api/types';
import type { Translator } from '../i18n';
import {
  draftKey,
  emptyResourceDraft,
  isBlank,
  patchValue,
  resourceDraftFrom,
  valueErrors,
} from './draft';
import type { ResourceDraft, ValueDraft, ValueKind } from './draft';
import type { TermChoice } from '../pages/term-lists/term-picker';
import type { Termed } from './plain-terms';

/** What one template row says about its property's values. */
export interface FieldRule {
  readonly isRequired: boolean;
  readonly isPrivate: boolean;
  /** The data types a value may take; empty means any of the core three. */
  readonly dataTypes: readonly string[];
  readonly defaultLang: string | null;
}

/** One property of the form with the values the draft holds for it. */
export interface PropertyField {
  readonly propertyId: PropertyId;
  readonly term: string;
  /** The template's alternate label, else the property label, else the term. */
  readonly label: string;
  /** The template's alternate comment, else the property comment. */
  readonly comment: string | null;
  /** The property itself, whose stored comment is the term's definition. */
  readonly property: Termed;
  /** `null` for a property the template does not list. */
  readonly rule: FieldRule | null;
  readonly values: readonly ValueDraft[];
}

/** The data types an unconstrained field offers (`makeNewField`'s default). */
export const CORE_TYPES: readonly string[] = ['literal', 'uri', 'resource'];

/**
 * The editable shape of a data type name. A module's type renders as the kind
 * its prefix names, and a name with no known prefix edits as text.
 */
export const baseKind = (type: string): ValueKind => {
  if (type === 'uri' || type.startsWith('uri:')) {
    return 'uri';
  }
  if (type === 'resource' || type.startsWith('resource:')) {
    return 'resource';
  }
  return 'literal';
};

const nonEmpty = (text: string | null): string | null =>
  text === null || text.trim() === '' ? null : text;

export const templateById = (
  templates: readonly ResourceTemplate[],
  id: TemplateId | null,
): ResourceTemplate | null => templates.find((entry) => entry.id === id) ?? null;

/** What laying out a form reads of a template: its rows. */
export type TemplateLayout = Pick<ResourceTemplate, 'properties'>;

interface TemplateRow {
  readonly property: Property;
  readonly label: string | null;
  readonly comment: string | null;
  readonly rule: FieldRule;
}

/** The template's rows whose property the property list names, in template order. */
const templateRows = (
  template: TemplateLayout | null,
  properties: readonly Property[],
): readonly TemplateRow[] =>
  (template?.properties ?? []).flatMap((entry) => {
    const property = properties.find((candidate) => candidate.id === entry.property.id);
    return property === undefined
      ? []
      : [
          {
            property,
            label: nonEmpty(entry.alternateLabel),
            comment: nonEmpty(entry.alternateComment),
            rule: {
              isRequired: entry.isRequired,
              isPrivate: entry.isPrivate,
              dataTypes: entry.dataTypes,
              defaultLang: nonEmpty(entry.defaultLang),
            },
          },
        ];
  });

/**
 * The fields the form shows: every template row in template order, then each
 * other property in the order its first value appears (`applyResourceTemplate`
 * prepends the template's fields and leaves the rest after them).
 */
export const layoutFields = (
  values: readonly ValueDraft[],
  template: TemplateLayout | null,
  properties: readonly Property[],
): readonly PropertyField[] => {
  const valuesOf = (id: PropertyId): readonly ValueDraft[] =>
    values.filter((value) => value.propertyId === id);
  const rows = templateRows(template, properties);
  const fromTemplate = rows.map(
    (row): PropertyField => ({
      propertyId: row.property.id,
      term: row.property.term,
      label: row.label ?? row.property.label ?? row.property.term,
      comment: row.comment ?? row.property.comment,
      property: row.property,
      rule: row.rule,
      values: valuesOf(row.property.id),
    }),
  );
  const listed = new Set<PropertyId>(rows.map((row) => row.property.id));
  const others: PropertyField[] = [];
  for (const value of values) {
    if (listed.has(value.propertyId)) {
      continue;
    }
    listed.add(value.propertyId);
    const property = properties.find((candidate) => candidate.id === value.propertyId);
    others.push({
      propertyId: value.propertyId,
      term: value.term,
      label: property?.label ?? value.term,
      comment: property?.comment ?? null,
      property: property ?? { term: value.term, localName: value.term, label: null, comment: null },
      rule: null,
      values: valuesOf(value.propertyId),
    });
  }
  return [...fromTemplate, ...others];
};

/** `makeNewValue`: the row's first data type, its default language and its visibility. */
const blankValue = (
  property: Pick<Property, 'id' | 'term'>,
  rule: FieldRule | null,
): ValueDraft => {
  const type = rule?.dataTypes[0] ?? 'literal';
  return {
    key: draftKey(),
    propertyId: property.id,
    term: property.term,
    type,
    kind: baseKind(type),
    text: '',
    label: '',
    lang: rule?.defaultLang ?? '',
    isPublic: !(rule?.isPrivate ?? false),
  };
};

/** One more empty value on a field, with the field's defaults. */
export const addFieldValue = (draft: ResourceDraft, field: PropertyField): ResourceDraft => ({
  ...draft,
  values: [...draft.values, blankValue({ id: field.propertyId, term: field.term }, field.rule)],
});

/** An empty value for a property the template does not list. */
export const addPropertyValue = (draft: ResourceDraft, property: Property): ResourceDraft => ({
  ...draft,
  values: [...draft.values, blankValue(property, null)],
});

/**
 * `rewritePropertyField`'s "add an empty value if none already exist": each
 * template row with no value gets one, so the form shows an input for it.
 */
export const withTemplateRows = (
  draft: ResourceDraft,
  template: TemplateLayout | null,
  properties: readonly Property[],
): ResourceDraft => {
  const missing = templateRows(template, properties).filter(
    (row) => !draft.values.some((value) => value.propertyId === row.property.id),
  );
  return {
    ...draft,
    values: [...draft.values, ...missing.map((row) => blankValue(row.property, row.rule))],
  };
};

/**
 * The draft after the user picks a template. The class follows the type while
 * the user has not chosen another: it is set when the resource has none or
 * still has the previous type's class, and kept when it was overridden.
 * (Upstream's `applyResourceTemplate(true)` sets it only while the class is
 * empty; with the class behind an "Advanced" fold, a type picked by mistake
 * would otherwise leave its class behind.) Every empty value goes and each
 * template row without a value gets a fresh one, as `rewritePropertyField`
 * removes the untouched default values and starts the row again with its
 * defaults; a typed value always stays. Choosing no template changes nothing
 * but the template.
 */
export const withTemplate = (
  draft: ResourceDraft,
  template: ResourceTemplate | null,
  properties: readonly Property[],
  previous: ResourceTemplate | null = null,
): ResourceDraft => {
  if (template === null) {
    return { ...draft, resourceTemplate: null };
  }
  const followsType =
    draft.resourceClass === null || draft.resourceClass === (previous?.resourceClass?.id ?? null);
  return withTemplateRows(
    {
      ...draft,
      resourceTemplate: template.id,
      resourceClass: followsType ? (template.resourceClass?.id ?? null) : draft.resourceClass,
      values: draft.values.filter((value) => !isBlank(value)),
    },
    template,
    properties,
  );
};

/** `initPage` on an add form: an empty title and an empty description. */
export const newResourceDraft = (properties: readonly Property[]): ResourceDraft => {
  const draft = emptyResourceDraft();
  const defaults = ['dcterms:title', 'dcterms:description'].flatMap((term) => {
    const property = properties.find((candidate) => candidate.term === term);
    return property === undefined ? [] : [blankValue(property, null)];
  });
  return { ...draft, values: defaults };
};

/**
 * The required fields with no filled value (the submit handler's check, and
 * `AbstractResourceEntityAdapter::validateEntity` on the server).
 */
export const missingRequired = (fields: readonly PropertyField[]): readonly PropertyField[] =>
  fields.filter(
    (field) => field.rule?.isRequired === true && field.values.every((value) => isBlank(value)),
  );

/** Every reason the form refuses to send the draft, required fields first. */
export const draftErrors = (
  draft: ResourceDraft,
  templates: readonly ResourceTemplate[],
  properties: readonly Property[],
  t: Translator['t'],
): readonly string[] => [
  ...missingRequired(
    layoutFields(draft.values, templateById(templates, draft.resourceTemplate), properties),
  ).map((field) => t('valueField.requiredError', { field: field.label })),
  ...valueErrors(draft.values, t),
];

/**
 * The draft a form opens with: a stored resource with its template's rows laid
 * in, or a new one with the add form's defaults.
 */
export const openDraft = (
  entity: ResourceEntity<number> | null,
  templates: readonly ResourceTemplate[],
  properties: readonly Property[],
): ResourceDraft => {
  if (entity === null) {
    return newResourceDraft(properties);
  }
  const draft = resourceDraftFrom(entity);
  return withTemplateRows(draft, templateById(templates, draft.resourceTemplate), properties);
};

/**
 * The data types one value may switch to: the row's list, or the core three
 * when it has none. A stored type outside that list stays offered, so opening
 * a value never changes its type.
 */
export const typeChoices = (field: PropertyField, value: ValueDraft): readonly string[] => {
  const allowed = field.rule === null || field.rule.dataTypes.length === 0
    ? CORE_TYPES
    : field.rule.dataTypes;
  return allowed.includes(value.type) ? allowed : [...allowed, value.type];
};

export const setValueType = (draft: ResourceDraft, key: string, type: string): ResourceDraft =>
  patchValue(draft, key, { type, kind: baseKind(type) });

/**
 * The input a value edits through. A term list value is picked from its list
 * whatever shape its entries take; any other value types into its base kind.
 * A list the server no longer lists edits as its base kind too, the way
 * upstream renders a type it has no service for (`DataType\Fallback`).
 */
export type ValueInput =
  | { readonly kind: 'term-list'; readonly termList: TermListName }
  | { readonly kind: ValueKind };

export const valueInput = (
  value: ValueDraft,
  installed: readonly Pick<DataType, 'id'>[],
): ValueInput =>
  isTermListName(value.type) && installed.some((dataType) => dataType.id === value.type)
    ? { kind: 'term-list', termList: value.type }
    : { kind: value.kind };

/**
 * A picked entry in the shape it is stored as: the word, the URI with its
 * label, or the item id. The type stays the list's name; `null` clears it.
 */
export const pickTerm = (
  draft: ResourceDraft,
  key: string,
  choice: TermChoice | null,
): ResourceDraft =>
  choice === null
    ? patchValue(draft, key, { text: '', label: '' })
    : patchValue(draft, key, {
        kind: choice.kind,
        text: choice.value,
        label: choice.kind === 'uri' && choice.label !== choice.value ? choice.label : '',
      });
