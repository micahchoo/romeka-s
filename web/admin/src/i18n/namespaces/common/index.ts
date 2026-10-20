/**
 * The shared page messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `CommonMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  "type.label": "Type",
  "type.none": "No type",
  "value.kind": "Kind",
  "type.classLine": "Standard class: {name}",
  "advanced.summary": "Advanced",
  "advanced.classDiffers": "This class is not the one its type suggests.",
  "help.what": "What is {name}?",
  "help.none": "No definition is stored for this term.",
  "picker.matches": "Matches for “{query}”",
  "picker.none": "Nothing matches “{query}”.",
  "picker.create": "Create a new field “{name}”",
  'page.vocabularies': 'Vocabularies',
  // Shared words and fallback markers.
  'common.yes': 'Yes',
  'common.no': 'No',
  'common.public': 'Public',
  'common.private': 'Private',
  'common.none': '[None]',
  'common.noOwner': '[No owner]',
  'common.untitled': '[Untitled]',
  'common.unknown': '[Unknown]',
  'common.never': '[Never]',
  // Shared actions.
  'action.save': 'Save',
  'action.cancel': 'Cancel',
  'action.remove': 'Remove',
  'action.previous': 'Previous',
  'action.next': 'Next',
  // Shared list surface (`src/app/list.tsx`).
  'list.loading': 'Loading {resource}...',
  'list.pager': 'Page {page} of {lastPage} ({total} {resource}) ',
  'list.empty': 'Romeka could not find any {resource}.',
  'list.tableLabel': '{resource} table',
  'loading.form': 'Loading form...',
  // Shared delete control (`src/app/delete.tsx`).
  'delete.confirm': 'Delete {name}? This cannot be undone.',
  'delete.button': 'Delete {name}',
  'delete.busy': 'Deleting...',
  // Sign-in surface (`src/app/auth.tsx`, `src/app/password.tsx`,
  // `src/app/forgot.tsx`). `auth.heading` names the key form, which the e2e and
  // accessibility specs address by that name.
  'auth.panelLabel': 'Sign in',
  // The write form fields (`src/app/fields.tsx`).
  'field.resourceLegend': 'Resource',
  'field.class': 'Class',
  'field.owner': 'Owner',
  'field.public': 'Public',
  'field.valuesLegend': 'Values',
  'field.addValue': 'Add value',
  'field.uri': 'URI',
  'field.resourceId': 'Resource ID',
  'field.label': 'Label',
  'field.language': 'Language',
  'field.removeValue': 'Remove {term} value',
  'field.kindText': 'Text',
  'field.kindUri': 'URI',
  'field.kindResource': 'Resource',
  'field.saved': 'Saved.',
  'form.errorEmptyLabel': 'The label cannot be empty.',
  // Table headers, shared across browse tables.
  'column.id': 'ID',
  'column.title': 'Title',
  'column.label': 'Label',
  'column.owner': 'Owner',
  'column.created': 'Created',
  'column.public': 'Public',
  'column.open': 'Open',
  'column.mediaType': 'Media type',
  'column.class': 'Class',
  'column.preview': 'Preview',
  'column.item': 'Item',
  'column.source': 'Source',
  'column.ingester': 'Ingester',
  'column.resourceClass': 'Resource class',
  'resource.itemSets': 'item sets',
  'resource.media': 'media',
  'resource.items': 'items',
  // Draft validation (`src/app/draft.ts`).
  'draft.needsResourceId': '{term} needs a positive resource id.',
  // The value editor a resource template lays out (vocab/template-editor).
  // `valueField.requiredError` is upstream's resource-form.js submit message.
  'valueField.requiredError': 'The following field is required: {field}',
  'valueField.required': 'Required',
  'valueField.private': 'Private',
  'valueField.privateHint': 'The template makes new values of this field private.',
  'valueField.dataType': '{field} data type',
  'valueField.uri': '{field} URI',
  'valueField.uriLabel': '{field} URI label',
  'valueField.resourceId': '{field} resource ID',
  'valueField.language': '{field} language',
  'valueField.public': '{field} value public',
  'valueField.addValue': 'Add value to {field}',
  'valueField.addField': 'Add another field',
  'valueField.addFieldHint': 'Describe it, or type a term',
  "tags.legend": "Tags",
  "tags.add": "Add a tag",
  "tags.hint": "Free words to find things by later. Press Enter or type a comma to add one.",
  "tags.remove": "Remove the tag “{tag}”",
  // Reorderable lists (app/reorder).
  'reorder.handle': 'Move {label}, {position} of {count}',
  'reorder.grip': 'Drag to move, or press Space to pick up',
  'reorder.up': 'Move {label} up',
  'reorder.down': 'Move {label} down',
  'reorder.upTip': 'Move up',
  'reorder.downTip': 'Move down',
  'reorder.picked': 'Picked up {label}. Use the arrow keys to move it, Space to drop it, Escape to cancel.',
  'reorder.moved': '{label}: position {position} of {count}.',
  'reorder.dropped': 'Dropped {label} at position {position} of {count}.',
  'reorder.cancelled': 'Move of {label} cancelled.',
} as const;

/** A complete table of this namespace, in one locale. */
export type CommonMessages = Table<keyof typeof en>;

export const common = defineNamespace('common', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
