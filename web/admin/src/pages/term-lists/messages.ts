/**
 * The Term lists page's messages and the term picker's, in English and Spanish.
 *
 * They live beside the page rather than in `src/i18n/catalogue.ts` because the
 * main catalogues load with the first paint and their budget is tight
 * (`perf/budget.json`). Here they load with the page chunk. `es` is typed
 * against `en`, so a key added in English and missing in Spanish is a `tsc`
 * error.
 */
import { useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale, MessageParams } from '../../i18n';

const en = {
  'termLists.resource': 'term lists',
  'termLists.invalidId': '"{param}" is not a term list id.',
  'termLists.intro': 'A term list turns a field into a choice from a fixed set of entries, so the same place or subject is always spelled the same way.',
  'termLists.add': 'Add term list',
  'termLists.legend': 'Term list',
  'termLists.kindLegend': 'What the list holds',
  'termLists.kindTerms': 'A list of words',
  'termLists.kindItems': 'Items from an item set',
  'termLists.kindUris': 'Links (URIs) with labels',
  'termLists.kindEmpty': 'Nothing yet',
  'termLists.termsLabel': 'Words, one per line',
  'termLists.urisLabel': 'Links, one per line: the URI, a space, then the label',
  'termLists.itemSetLabel': 'Item set',
  'termLists.selectItemSet': 'Select an item set',
  'termLists.langLabel': 'Language of the words',
  'termLists.langHint': 'Optional. Every word picked from the list carries this language tag, for example en.',
  'termLists.errorNoEntries': 'Add at least one entry.',
  'termLists.errorNoItemSet': 'Choose an item set.',
  'termLists.columnKind': 'Holds',
  'termLists.columnEntries': 'Entries',
  'termLists.columnUsage': 'Items using it',
  'termLists.editLabel': 'Edit this term list',
  'termLists.deleteLabel': 'Delete this term list',
  'termLists.deleteNote': 'Deleting a list keeps the values chosen from it. They stay on their items as plain text.',
  'termPicker.loading': 'Loading the list...',
  'termPicker.noMatch': 'No entry matches "{query}".',
  'termPicker.notInList': '"{value}" is not in this list. Pick an entry to replace it.',
  'termPicker.hint': 'Type to filter the {list} list, then pick an entry.',
  'termPicker.typeLabel': 'Term list: {list}',
} as const;

/** A Term lists message id. */
export type TermListMessageKey = keyof typeof en;

const es: Readonly<Record<TermListMessageKey, string>> = {
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

const MESSAGES: Readonly<Record<Locale, Readonly<Record<TermListMessageKey, string>>>> = { en, es };

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The Term lists message lookup in the current locale. */
export const useTermListText = (): ((
  key: TermListMessageKey,
  params?: MessageParams,
) => string) => {
  const locale = useLocale();
  return useMemo(() => {
    const messages = MESSAGES[locale];
    return (key: TermListMessageKey, params?: MessageParams): string =>
      interpolate(messages[key], params);
  }, [locale]);
};
