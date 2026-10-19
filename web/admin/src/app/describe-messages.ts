/**
 * The strings the item, item set and media forms share with the item type
 * pages: the Type picker, the "Advanced" fold, the term picker and "?". A
 * page-local table (`local-messages.ts`), so they ship with the forms' chunk and
 * not in the initial payload.
 */
import type { LocalMessages } from './local-messages';

const en = {
  'type.label': 'Type',
  'type.none': 'No type',
  'value.kind': 'Kind',
  'type.classLine': 'Standard class: {name}',
  'advanced.summary': 'Advanced',
  'advanced.classDiffers': 'This class is not the one its type suggests.',
  'help.what': 'What is {name}?',
  'help.none': 'No definition is stored for this term.',
  'picker.matches': 'Matches for “{query}”',
  'picker.none': 'Nothing matches “{query}”.',
  'picker.create': 'Create a new field “{name}”',
} as const;

type Key = keyof typeof en;

export const DESCRIBE: LocalMessages<Key> = {
  en,
  es: {
    'type.label': 'Tipo',
    'type.none': 'Sin tipo',
    'value.kind': 'Tipo de valor',
    'type.classLine': 'Clase estándar: {name}',
    'advanced.summary': 'Avanzado',
    'advanced.classDiffers': 'Esta clase no es la que sugiere su tipo.',
    'help.what': '¿Qué es {name}?',
    'help.none': 'No hay una definición guardada para este término.',
    'picker.matches': 'Resultados para «{query}»',
    'picker.none': 'Nada coincide con «{query}».',
    'picker.create': 'Crear un campo nuevo «{name}»',
  },
};
