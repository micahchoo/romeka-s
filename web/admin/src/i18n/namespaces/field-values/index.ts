/**
 * The values board's messages (`#/fields/<id>`): one field's values, and moving a value to
 * another field.
 *
 * English is here, in the chunk the board loads; every other locale is its own chunk under
 * `locales/`, fetched the first time the board reads it in that locale. Each locale file is typed
 * `FieldValuesMessages`, so a key added here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'values.title': 'Values of {field}',
  'values.allFields': 'All fields',
  'values.intro':
    'Every text this field holds, on items, item sets and media, most used first. Drag a value to another field, or use “Move to”, and it moves on every record that holds it.',
  'values.loading': 'Reading the values…',
  'values.noField': 'No field has the number {id}.',
  'values.addColumn': 'Add a field',
  'values.chooseField': 'Choose a field',
  'values.removeColumn': 'Remove the {field} column',
  'values.countOne': '1 value',
  'values.countMany': '{count} values',
  'values.find': 'Find a value',
  'values.empty': 'No record has a value in {field} yet.',
  'values.noMatch': 'No value holds “{text}”.',
  'values.dropHere': 'Drop values here to move them to {field}.',
  'values.showMore': 'Show {count} more',
  'values.andMore': 'and {count} more',
  'values.itemsOne': '1 item',
  'values.itemsMany': '{count} items',
  'values.mediaCount': '{count} media',
  'values.setsOne': '1 item set',
  'values.setsMany': '{count} item sets',
  'values.choose': 'Choose “{value}”',
  'values.chosen': '{count} values chosen. Drag one of them, or use “Move to” on one, to move them all.',
  'values.clearChosen': 'Clear the choice',
  'values.moveTo': 'Move to…',
  'values.moveLabel': 'Move “{value}” to',
  'values.show': 'Show records',
  'values.hide': 'Hide records',
  'values.reading': 'Reading the records…',
  'values.openItems': 'Open these items in Items',
  'values.readOnly': 'You cannot change these records.',
  'values.partEditable': 'You can change {editable} of {total}.',
  'values.checkTitle': 'Check the move',
  'values.checking': 'Checking what would change…',
  'values.previewOne': 'Move “{value}” from {from} to {to} on 1 record.',
  'values.previewMany': 'Move “{value}” from {from} to {to} on {count} records.',
  'values.merged': '{count} of them already hold it under {to}; they only lose it from {from}.',
  'values.skipped': '{count} records you cannot change keep it under {from}.',
  'values.nothing': 'Nothing to move: no record you can change holds “{value}” under {from}.',
  'values.notInForm':
    '{to} is not one of the fields of {type}: {count} of these records will hold a value its form does not show.',
  'values.addToType': 'Add {to} to {type}',
  'values.move': 'Move',
  'values.moving': 'Moving…',
  'values.doneOne': 'Moved “{value}” to {to} on {count} records.',
  'values.doneMany': 'Moved {values} values to {to} on {count} records.',
  'values.undo': 'Undo',
  'values.undoing': 'Putting it back…',
  'values.undone': 'Put “{value}” back under {from}.',
  'values.history': 'Moves on this visit',
  'values.historyRow': '“{value}”: {from} → {to}, {count} records',
  'values.undoLast': 'Undo the last move',
  'values.undoNote':
    'Undo puts back the last move made on this visit. The History of each item lists every move.',
} as const;

/** A complete table of this namespace, in one locale. */
export type FieldValuesMessages = Table<keyof typeof en>;

export const fieldValues = defineNamespace('field-values', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
