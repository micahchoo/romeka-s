/**
 * "What kind of thing is this?": from a typed kind to a starter, and from a
 * starter to a new type's draft. The starters themselves are the server's
 * (`api/starters.ts`); a kind they do not know starts as its `other` starter,
 * a physical object with a title, which fits anything one can hold.
 */
import type { Property, ResourceClass } from "../../api/types";
import type { Starter } from "../../api/starters";
import { type Locale } from "../../i18n";
import { termLabels } from "../../i18n/termText";
import { fieldKey, type FieldDraft, type TypeDraft } from "./model";
import { starterLabel } from "./starter-text";
import { storedDefaultName } from "../../app/plain-terms";

const fold = (text: string): string =>
  text.trim().toLowerCase().normalize("NFD").replace(/\p{M}/gu, "");

/** The starter a typed kind names, or `null` for a kind the table does not know. */
export const starterFor = (
  starters: readonly Starter[],
  kind: string,
): Starter | null => {
  const folded = fold(kind);
  return starters.find((starter) => starter.words.includes(folded)) ?? null;
};

/**
 * A new type's draft: the kind's class and fields, those this installation
 * has. An alternate label is kept only where it differs from the property's
 * own label as stored and as the locale shows it, so a "Title" row, or a
 * "Título" one in Spanish, reads the property's label.
 */
export const draftFromStarter = (
  name: string,
  starter: Starter,
  locale: Locale,
  properties: readonly Property[],
  classes: readonly ResourceClass[],
): TypeDraft => {
  const byTerm = (term: string): Property | undefined =>
    properties.find((property) => property.term === term);
  const fields = starter.fields.flatMap((entry): FieldDraft[] => {
    const property = byTerm(entry.term);
    if (property === undefined) {
      return [];
    }
    const label = starterLabel(entry, locale);
    const own = storedDefaultName(property, termLabels(locale));
    return [
      {
        key: fieldKey(),
        propertyId: property.id,
        alternateLabel: label === property.label || label === own ? null : label,
        alternateComment: null,
        dataTypes: entry.dataTypes,
        isRequired: entry.required,
        isPrivate: false,
        defaultLang: null,
      },
    ];
  });
  const first = starter.fields[0];
  return {
    label: name.trim(),
    owner: null,
    resourceClass:
      classes.find((entry) => entry.term === starter.classTerm)?.id ?? null,
    titleProperty: first === undefined ? null : (byTerm(first.term)?.id ?? null),
    descriptionProperty: null,
    fields,
  };
};
