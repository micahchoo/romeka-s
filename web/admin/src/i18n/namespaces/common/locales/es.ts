/**
 * The Spanish shared page messages, written by hand.
 *
 * It is typed `CommonMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { CommonMessages } from '../index';

export const es: CommonMessages = {
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
};
