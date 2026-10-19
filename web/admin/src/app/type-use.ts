/**
 * "Used for": which forms offer an item type. Upstream templates are not tied
 * to a resource kind and the template row has no member a client may extend,
 * so the hint lives in this browser's `localStorage` and nowhere on the
 * server. It only narrows the Type picker; the stored template, and every
 * other client, are untouched. A type with no stored hint is offered
 * everywhere.
 */
import type { ResourceTemplate, TemplateId } from '../api/types';

export type ResourceKind = 'items' | 'item-sets' | 'media';

export const RESOURCE_KINDS: readonly ResourceKind[] = ['items', 'item-sets', 'media'];

export const TYPE_USE_STORAGE = 'romeka.itemTypeUse';

const isKind = (value: unknown): value is ResourceKind =>
  RESOURCE_KINDS.some((kind) => kind === value);

/** The stored hints, narrowed; anything unreadable counts as no hint. */
const readAll = (): ReadonlyMap<string, readonly ResourceKind[]> => {
  let raw: unknown;
  try {
    raw = JSON.parse(window.localStorage.getItem(TYPE_USE_STORAGE) ?? '{}');
  } catch {
    return new Map();
  }
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return new Map();
  }
  return new Map(
    Object.entries(raw).flatMap(([id, kinds]): [string, readonly ResourceKind[]][] =>
      Array.isArray(kinds) ? [[id, kinds.filter(isKind)]] : [],
    ),
  );
};

export const usedFor = (id: TemplateId): readonly ResourceKind[] =>
  readAll().get(String(id)) ?? RESOURCE_KINDS;

export const setUsedFor = (id: TemplateId, kinds: readonly ResourceKind[]): void => {
  const others = [...readAll()].filter(([key]) => key !== String(id));
  const everywhere = RESOURCE_KINDS.every((kind) => kinds.includes(kind));
  const next = Object.fromEntries(
    everywhere
      ? others
      : [...others, [String(id), RESOURCE_KINDS.filter((kind) => kinds.includes(kind))]],
  );
  try {
    window.localStorage.setItem(TYPE_USE_STORAGE, JSON.stringify(next));
  } catch {
    // A browser that refuses storage keeps offering the type everywhere.
  }
};

/** The types a form of `kind` offers; the one the resource has stays offered. */
export const typesFor = (
  templates: readonly ResourceTemplate[],
  kind: ResourceKind,
  current: TemplateId | null,
): readonly ResourceTemplate[] => {
  const hints = readAll();
  return templates.filter(
    (template) =>
      template.id === current || (hints.get(String(template.id)) ?? RESOURCE_KINDS).includes(kind),
  );
};
