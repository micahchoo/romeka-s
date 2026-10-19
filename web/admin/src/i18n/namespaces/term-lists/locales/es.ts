/**
 * The Spanish Term lists messages, written by hand.
 *
 * It is typed `TermListMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { TermListMessages } from '../index';

export const es: TermListMessages = {
  'termLists.resource': 'listas de términos',
  'termLists.invalidId': '"{param}" no es un identificador de lista de términos.',
  'termLists.intro': 'Una lista de términos convierte un campo en una elección entre entradas fijas, para que el mismo lugar o tema se escriba siempre igual.',
  'termLists.add': 'Añadir lista de términos',
  'termLists.legend': 'Lista de términos',
  'termLists.kindLegend': 'Qué contiene la lista',
  'termLists.kindTerms': 'Una lista de palabras',
  'termLists.kindItems': 'Elementos de un conjunto de elementos',
  'termLists.kindUris': 'Enlaces (URI) con etiquetas',
  'termLists.kindEmpty': 'Nada todavía',
  'termLists.termsLabel': 'Palabras, una por línea',
  'termLists.urisLabel': 'Enlaces, uno por línea: la URI, un espacio y la etiqueta',
  'termLists.itemSetLabel': 'Conjunto de elementos',
  'termLists.selectItemSet': 'Seleccione un conjunto de elementos',
  'termLists.langLabel': 'Idioma de las palabras',
  'termLists.langHint': 'Opcional. Cada palabra elegida de la lista lleva esta etiqueta de idioma, por ejemplo es.',
  'termLists.errorNoEntries': 'Añada al menos una entrada.',
  'termLists.errorNoItemSet': 'Elija un conjunto de elementos.',
  'termLists.columnKind': 'Contiene',
  'termLists.columnEntries': 'Entradas',
  'termLists.columnUsage': 'Elementos que la usan',
  'termLists.editLabel': 'Editar esta lista de términos',
  'termLists.deleteLabel': 'Eliminar esta lista de términos',
  'termLists.deleteNote': 'Eliminar una lista conserva los valores elegidos en ella. Quedan en sus elementos como texto sin formato.',
  'termPicker.loading': 'Cargando la lista...',
  'termPicker.noMatch': 'Ninguna entrada coincide con "{query}".',
  'termPicker.notInList': '"{value}" no está en esta lista. Elija una entrada para reemplazarlo.',
  'termPicker.hint': 'Escriba para filtrar la lista {list} y elija una entrada.',
  'termPicker.typeLabel': 'Lista de términos: {list}',
};
