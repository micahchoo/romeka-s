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
