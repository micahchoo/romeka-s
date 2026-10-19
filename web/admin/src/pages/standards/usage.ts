/**
 * How many item types (resource templates) name each property and each class.
 * The API carries no such count, so it is tallied from every template, which a
 * collection keeps few of.
 */
import { api } from '../../api/client';
import type { ClassId, PropertyId } from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';

export interface TemplateUsage {
  readonly properties: ReadonlyMap<PropertyId, number>;
  readonly classes: ReadonlyMap<ClassId, number>;
}

const bump = <K>(counts: Map<K, number>, key: K): void => {
  counts.set(key, (counts.get(key) ?? 0) + 1);
};

export const loadTemplateUsage = async (): Promise<TemplateUsage> => {
  const templates = await allPages((page) =>
    api.listResourceTemplates({ page, perPage: ALL_PAGES_PER_PAGE }),
  );
  const properties = new Map<PropertyId, number>();
  const classes = new Map<ClassId, number>();
  for (const template of templates) {
    for (const id of new Set(template.properties.map((row) => row.property.id))) {
      bump(properties, id);
    }
    if (template.resourceClass !== null) {
      bump(classes, template.resourceClass.id);
    }
  }
  return { properties, classes };
};
