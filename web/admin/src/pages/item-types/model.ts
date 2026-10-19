/**
 * An item type is an upstream resource template seen in plain words. The draft
 * mirrors `ResourceTemplateWrite` member for member and keeps each stored value
 * as read, so a template made raw through the API (several data types in any
 * order, no class, unknown data types) saves back unchanged unless the user
 * edits it. Every edit is one function here; the editor only calls them.
 */
import type {
  ClassId,
  Property,
  PropertyId,
  ResourceTemplate,
  ResourceTemplateWrite,
  UserId,
} from '../../api/types';
import type { TemplateLayout } from '../../app/value-fields';

/** One template row: an `o:resource_template_property` entry. */
export interface FieldDraft {
  /** Stable React key; never sent. */
  readonly key: string;
  readonly propertyId: PropertyId;
  readonly alternateLabel: string | null;
  readonly alternateComment: string | null;
  /** Kept in stored order; an empty list accepts the core three. */
  readonly dataTypes: readonly string[];
  readonly isRequired: boolean;
  readonly isPrivate: boolean;
  readonly defaultLang: string | null;
}

export interface TypeDraft {
  readonly label: string;
  readonly owner: UserId | null;
  readonly resourceClass: ClassId | null;
  readonly titleProperty: PropertyId | null;
  readonly descriptionProperty: PropertyId | null;
  readonly fields: readonly FieldDraft[];
}

let nextKey = 0;

export const fieldKey = (): string => {
  nextKey += 1;
  return `type-field-${String(nextKey)}`;
};

export const typeDraftFrom = (template: ResourceTemplate): TypeDraft => ({
  label: template.label,
  owner: template.owner?.id ?? null,
  resourceClass: template.resourceClass?.id ?? null,
  titleProperty: template.titleProperty?.id ?? null,
  descriptionProperty: template.descriptionProperty?.id ?? null,
  fields: template.properties.map((entry) => ({
    key: fieldKey(),
    propertyId: entry.property.id,
    alternateLabel: entry.alternateLabel,
    alternateComment: entry.alternateComment,
    dataTypes: entry.dataTypes,
    isRequired: entry.isRequired,
    isPrivate: entry.isPrivate,
    defaultLang: entry.defaultLang,
  })),
});

export const toTemplateWrite = (draft: TypeDraft): ResourceTemplateWrite => ({
  label: draft.label,
  owner: draft.owner,
  resourceClass: draft.resourceClass,
  titleProperty: draft.titleProperty,
  descriptionProperty: draft.descriptionProperty,
  properties: draft.fields.map((field) => ({
    propertyId: field.propertyId,
    alternateLabel: field.alternateLabel,
    alternateComment: field.alternateComment,
    dataTypes: field.dataTypes,
    isRequired: field.isRequired,
    isPrivate: field.isPrivate,
    defaultLang: field.defaultLang,
  })),
});

/**
 * The draft as the rows the item form lays out, so the preview is the real
 * values editor reading the rows being edited.
 */
export const asLayout = (draft: TypeDraft): TemplateLayout => ({
  properties: draft.fields.map((field) => ({
    property: { id: field.propertyId, apiUrl: '' },
    alternateLabel: field.alternateLabel,
    alternateComment: field.alternateComment,
    dataTypes: field.dataTypes,
    isRequired: field.isRequired,
    isPrivate: field.isPrivate,
    defaultLang: field.defaultLang,
  })),
});

/** A typed text: an empty one clears the member, as the server would store it. */
export const textOrNull = (text: string): string | null => (text.trim() === '' ? null : text);

export const patchField = (
  draft: TypeDraft,
  key: string,
  patch: Partial<Omit<FieldDraft, 'key'>>,
): TypeDraft => ({
  ...draft,
  fields: draft.fields.map((field) => (field.key === key ? { ...field, ...patch } : field)),
});

/** A new row for `property`, at the end, accepting `dataTypes`. */
export const addField = (
  draft: TypeDraft,
  property: Pick<Property, 'id'>,
  dataTypes: readonly string[],
): TypeDraft => ({
  ...draft,
  fields: [
    ...draft.fields,
    {
      key: fieldKey(),
      propertyId: property.id,
      alternateLabel: null,
      alternateComment: null,
      dataTypes,
      isRequired: false,
      isPrivate: false,
      defaultLang: null,
    },
  ],
});

export const removeField = (draft: TypeDraft, key: string): TypeDraft => ({
  ...draft,
  fields: draft.fields.filter((field) => field.key !== key),
});

/** The row moved to `index`, the others keeping their order. */
export const moveField = (draft: TypeDraft, key: string, index: number): TypeDraft => {
  const moved = draft.fields.find((field) => field.key === key);
  if (moved === undefined) {
    return draft;
  }
  const rest = draft.fields.filter((field) => field.key !== key);
  const at = Math.max(0, Math.min(index, rest.length));
  return { ...draft, fields: [...rest.slice(0, at), moved, ...rest.slice(at)] };
};

/** Add or drop one accepted data type, keeping the order of the others. */
export const toggleDataType = (field: FieldDraft, type: string, on: boolean): readonly string[] =>
  on
    ? field.dataTypes.includes(type)
      ? field.dataTypes
      : [...field.dataTypes, type]
    : field.dataTypes.filter((entry) => entry !== type);

/** The properties a row may switch to: its own and those no other row holds. */
export const freeProperties = (
  draft: TypeDraft,
  properties: readonly Property[],
  own: PropertyId | null,
): readonly Property[] =>
  properties.filter(
    (property) =>
      property.id === own || !draft.fields.some((field) => field.propertyId === property.id),
  );
