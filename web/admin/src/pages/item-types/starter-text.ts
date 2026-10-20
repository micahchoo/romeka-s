/**
 * Starter names and alternate labels in the locales plain words are curated
 * in. The server sends English; a locale with no entry here reads that. Names
 * are keyed by the server's `nameKey` and labels by their English text, so a
 * label the server renames reads English until this table follows.
 */
import { curatedLocale } from "../../app/plain-terms";
import { type Locale } from "../../i18n";
import type { Starter, StarterField } from "../../api/starters";

export const SPANISH_NAMES: Readonly<Record<string, string>> = {
  "starter.letter": "Carta",
  "starter.photograph": "Fotografía",
  "starter.postcard": "Postal",
  "starter.diary": "Diario",
  "starter.person": "Persona",
  "starter.other": "Objeto físico",
};

export const SPANISH_LABELS: Readonly<Record<string, string>> = {
  Title: "Título",
  Identifier: "Identificador",
  Sender: "Remitente",
  Recipient: "Destinatario",
  "Date written": "Fecha de escritura",
  Place: "Lugar",
  Description: "Descripción",
  Photographer: "Fotógrafo",
  "Date taken": "Fecha de la foto",
  "Who is in it": "Quién aparece",
  Rights: "Derechos",
  "Date sent": "Fecha de envío",
  "Postmark place": "Lugar del matasellos",
  Writer: "Autor",
  "Dates covered": "Fechas que abarca",
  "Number of pages": "Número de páginas",
  Name: "Nombre",
  Relationship: "Parentesco",
  Born: "Nació",
};

export const starterName = (starter: Starter, locale: Locale): string =>
  (curatedLocale(locale) === "es" ? SPANISH_NAMES[starter.nameKey] : undefined) ??
  starter.name;

export const starterLabel = (entry: StarterField, locale: Locale): string =>
  (curatedLocale(locale) === "es" ? SPANISH_LABELS[entry.label] : undefined) ??
  entry.label;
