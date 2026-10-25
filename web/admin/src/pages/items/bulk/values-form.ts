/**
 * The Values form's empty state and whether it holds a change. The edit step holds the form, and
 * this module is apart from `values-model.ts` so the panel chunk stays inside its budget
 * (`perf/budget.json`) while the rest of the form loads with the Values section.
 */
import type { Composer } from './values-model';

export const NEW_COMPOSER: Composer = {
  property: null,
  action: 'add',
  kind: 'literal',
  text: '',
  lang: '',
  uri: '',
  label: '',
  item: '',
  isPublic: true,
  from: '',
  to: '',
};

/**
 * Whether the form holds a change, whole or not, that the person has not added yet. A field
 * chosen is the start of one; Preview waits until it is added or cleared, so none is lost.
 */
export const isWriting = (composer: Composer): boolean => composer.property !== null;
