/**
 * The strings the item, item set and media forms share with the item type
 * pages: the Type picker, the "Advanced" fold, the term picker and "?". They
 * load with the forms' chunk rather than with the main catalogue, which every
 * first paint pays for (`perf/budget.json`). Eleven strings a locale is too
 * little to fetch on its own, so every locale is imported here, as the
 * Standards page does. Each table is typed against `en`, so a key added in
 * English and missing elsewhere is a `tsc` error.
 */
import { useMemo } from "react";
import { useLocale } from "../i18n";
import type { Locale, MessageParams } from "../i18n";
import { hi } from "./describe-locales/hi";
import { kn } from "./describe-locales/kn";
import { ml } from "./describe-locales/ml";
import { mr } from "./describe-locales/mr";
import { ne } from "./describe-locales/ne";
import { ta } from "./describe-locales/ta";
import { te } from "./describe-locales/te";

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
  "tags.legend": "Tags",
  "tags.add": "Add a tag",
  "tags.hint": "Free words to find things by later. Press Enter or type a comma to add one.",
  "tags.remove": "Remove the tag “{tag}”",
} as const;

type DescribeMessageKey = keyof typeof en;

/** A complete table of the shared describe strings. */
export type DescribeMessages = Readonly<Record<DescribeMessageKey, string>>;

const es: DescribeMessages = {
  "type.label": "Tipo",
  "type.none": "Sin tipo",
  "value.kind": "Tipo de valor",
  "type.classLine": "Clase estándar: {name}",
  "advanced.summary": "Avanzado",
  "advanced.classDiffers": "Esta clase no es la que sugiere su tipo.",
  "help.what": "¿Qué es {name}?",
  "help.none": "No hay una definición guardada para este término.",
  "picker.matches": "Resultados para «{query}»",
  "picker.none": "Nada coincide con «{query}».",
  "picker.create": "Crear un campo nuevo «{name}»",
  "tags.legend": "Etiquetas",
  "tags.add": "Añadir una etiqueta",
  "tags.hint": "Palabras libres para encontrar las cosas más tarde. Pulse Intro o escriba una coma para añadir una.",
  "tags.remove": "Quitar la etiqueta «{tag}»",
};

const MESSAGES: Readonly<Record<Locale, DescribeMessages>> = {
  en,
  es,
  hi,
  mr,
  ml,
  ta,
  kn,
  te,
  ne,
};

const interpolate = (
  template: string,
  params: MessageParams | undefined,
): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The shared describe strings' lookup in the current locale. */
export const useDescribeText = (): ((
  key: DescribeMessageKey,
  params?: MessageParams,
) => string) => {
  const locale = useLocale();
  return useMemo(() => {
    const messages = MESSAGES[locale];
    return (key: DescribeMessageKey, params?: MessageParams): string =>
      interpolate(messages[key], params);
  }, [locale]);
};
