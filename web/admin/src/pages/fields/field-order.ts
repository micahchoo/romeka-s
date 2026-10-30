/**
 * The fields an archive offers, in the order the Fields page and the values board list them: the
 * fields item types use first, most used first, then the common standard ones, then the rest.
 */
import { api } from '../../api/client';
import type { Property, ResourceTemplate } from '../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../app/all-pages';
import { PLAIN_TERMS, plainName } from '../../app/plain-terms';
import type { TermLabels } from '../../i18n';

export interface Use {
  readonly template: ResourceTemplate;
  /** The type's own name for the field, when it renames it. */
  readonly label: string | null;
}

export interface Loaded {
  readonly properties: readonly Property[];
  readonly templates: readonly ResourceTemplate[];
}

export const load = async (): Promise<Loaded> => {
  const [properties, templates] = await Promise.all([
    allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
  ]);
  return { properties, templates };
};

export const usesOf = (templates: readonly ResourceTemplate[], property: Property): readonly Use[] =>
  templates.flatMap((template) =>
    template.properties
      .filter((row) => row.property.id === property.id)
      .map((row) => ({ template, label: row.alternateLabel })),
  );

/** In use first (most used first), then the curated common fields, then by name. */
export const browseOrder = (loaded: Loaded, text: TermLabels): readonly Property[] => {
  const rank = (property: Property): number =>
    usesOf(loaded.templates, property).length * 10 + (property.term in PLAIN_TERMS ? 1 : 0);
  return [...loaded.properties].sort(
    (a, b) => rank(b) - rank(a) || plainName(a, text).localeCompare(plainName(b, text), text.locale),
  );
};

