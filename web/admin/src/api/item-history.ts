/**
 * An item's history (`GET /api/romeka/items/{id}/history`, `crates/omeka-api/src/read/item_history.rs`):
 * one entry per write that changed the item, newest first, each naming when, who, what made it,
 * and every changed field before and after. The server answers only a caller who may edit the
 * item, and leaves out what that caller may not see.
 *
 * A value arrives in the members the item write reads, so "Put this value back" sends it
 * through the ordinary item write as a {@link ValueWrite}.
 */
import { api } from './client';
import { list, member, nullable, oneOf, strict } from './import-read';
import { ParseError, asBoolean, asIdNumber, asRecord, asString, brandId } from './parse';
import type { ClassId, Id, ItemId, ItemSetId, Parser, Path, PropertyId, ResourceId, SiteId, TemplateId, UserId, ValueWrite } from './types';

export const RUN_KINDS = ['bulk', 'import', 'bulk_undo', 'import_undo'] as const;
export type RunKind = (typeof RUN_KINDS)[number];

/** What made a write: an edit, or the bulk run or import (or the undo of one) it names. */
export type HistorySource = { readonly kind: 'edit' } | { readonly kind: RunKind; readonly id: number };

/** One value as the caller may see it: its words, and the write that puts it back. */
export interface HistoryValue {
  readonly display: string;
  readonly write: ValueWrite;
}

export interface Labelled<IdT extends number> {
  readonly id: IdT;
  readonly label: string | null;
}

export interface Titled<IdT extends number> {
  readonly id: IdT;
  readonly title: string | null;
}

export const GONE_STATES = ['trash', 'deleted'] as const;

/**
 * An item set in a change. `gone` is `trash` for a set in the trash (with its title), `deleted`
 * for one deleted for good (no title), and null for a set in place.
 */
export interface HistorySet extends Titled<ItemSetId> {
  readonly gone: (typeof GONE_STATES)[number] | null;
}

export type HistoryChange =
  | {
      readonly field: 'values';
      readonly propertyId: PropertyId;
      readonly term: string;
      readonly label: string;
      readonly before: readonly HistoryValue[];
      readonly after: readonly HistoryValue[];
    }
  /** `before` is null where the item did not exist before the write. */
  | { readonly field: 'visibility'; readonly before: boolean | null; readonly after: boolean }
  | { readonly field: 'class'; readonly before: Labelled<ClassId> | null; readonly after: Labelled<ClassId> | null }
  | {
      readonly field: 'template';
      readonly before: Labelled<TemplateId> | null;
      readonly after: Labelled<TemplateId> | null;
    }
  | {
      readonly field: 'item_sets';
      readonly before: readonly HistorySet[];
      readonly after: readonly HistorySet[];
    }
  | { readonly field: 'sites'; readonly before: readonly Titled<SiteId>[]; readonly after: readonly Titled<SiteId>[] };

export interface HistoryEntry {
  readonly id: number;
  readonly created: string;
  /** `null` once the person's account is deleted. */
  readonly user: { readonly id: UserId; readonly name: string } | null;
  readonly source: HistorySource;
  readonly changes: readonly HistoryChange[];
}

const SOURCE_KINDS = ['edit', ...RUN_KINDS] as const;

const parseSource = (raw: unknown, path: Path): HistorySource => {
  const object = strict(raw, path, ['kind', 'id']);
  const kind = member(object, 'kind', path, oneOf(SOURCE_KINDS));
  if (kind === 'edit') {
    return { kind };
  }
  return { kind, id: member(object, 'id', path, asIdNumber) };
};

const optionalText = (object: Record<string, unknown>, key: string, path: Path): string | null => {
  const value = object[key];
  return value === undefined ? null : asString(value, [...path, key]);
};

/** A value in the item write's members, read for the property it belongs to. */
const parseValue =
  (term: string) =>
  (raw: unknown, path: Path): HistoryValue => {
    const object = asRecord(raw, path);
    const common = {
      term,
      propertyId: brandId<'PropertyId'>(member(object, 'property_id', path, asIdNumber)),
      type: member(object, 'type', path, asString),
      isPublic: member(object, 'is_public', path, asBoolean),
    };
    const display = member(object, 'display', path, asString);
    if ('value_resource_id' in object) {
      const resourceId = brandId<'ResourceId'>(member(object, 'value_resource_id', path, asIdNumber)) satisfies ResourceId;
      return { display, write: { ...common, kind: 'resource', resourceId } };
    }
    if ('@id' in object) {
      return {
        display,
        write: {
          ...common,
          kind: 'uri',
          uri: member(object, '@id', path, asString),
          label: optionalText(object, 'o:label', path),
          lang: optionalText(object, 'o:lang', path),
        },
      };
    }
    if ('@value' in object) {
      return {
        display,
        write: { ...common, kind: 'literal', text: member(object, '@value', path, asString), lang: optionalText(object, '@language', path) },
      };
    }
    throw new ParseError(path, 'a value with @value, @id or value_resource_id');
  };

const parseLabelled =
  <B extends string>() =>
  (raw: unknown, path: Path): Labelled<Id<B>> => {
    const object = strict(raw, path, ['id', 'label']);
    return {
      id: brandId<B>(member(object, 'id', path, asIdNumber)),
      label: member(object, 'label', path, nullable(asString)),
    };
  };

const parseTitled =
  <B extends string>() =>
  (raw: unknown, path: Path): Titled<Id<B>> => {
    const object = strict(raw, path, ['id', 'title']);
    return {
      id: brandId<B>(member(object, 'id', path, asIdNumber)),
      title: member(object, 'title', path, nullable(asString)),
    };
  };

const parseSet = (raw: unknown, path: Path): HistorySet => {
  const object = strict(raw, path, ['id', 'title'], ['state']);
  return {
    id: brandId<'ItemSetId'>(member(object, 'id', path, asIdNumber)),
    title: member(object, 'title', path, nullable(asString)),
    gone: 'state' in object ? member(object, 'state', path, oneOf(GONE_STATES)) : null,
  };
};

const FIELDS = ['values', 'visibility', 'class', 'template', 'item_sets', 'sites'] as const;

const parseChange = (raw: unknown, path: Path): HistoryChange => {
  const object = asRecord(raw, path);
  const field = member(object, 'field', path, oneOf(FIELDS));
  switch (field) {
    case 'values': {
      const term = member(object, 'term', path, asString);
      return {
        field,
        propertyId: brandId<'PropertyId'>(member(object, 'property_id', path, asIdNumber)),
        term,
        label: member(object, 'label', path, asString),
        before: member(object, 'before', path, list(parseValue(term))),
        after: member(object, 'after', path, list(parseValue(term))),
      };
    }
    case 'visibility':
      return {
        field,
        before: member(object, 'before', path, nullable(asBoolean)),
        after: member(object, 'after', path, asBoolean),
      };
    case 'class':
      return {
        field,
        before: member(object, 'before', path, nullable(parseLabelled<'ClassId'>())),
        after: member(object, 'after', path, nullable(parseLabelled<'ClassId'>())),
      };
    case 'template':
      return {
        field,
        before: member(object, 'before', path, nullable(parseLabelled<'TemplateId'>())),
        after: member(object, 'after', path, nullable(parseLabelled<'TemplateId'>())),
      };
    case 'item_sets':
      return {
        field,
        before: member(object, 'before', path, list(parseSet)),
        after: member(object, 'after', path, list(parseSet)),
      };
    case 'sites':
      return {
        field,
        before: member(object, 'before', path, list(parseTitled<'SiteId'>())),
        after: member(object, 'after', path, list(parseTitled<'SiteId'>())),
      };
  }
};

const parseUser = (raw: unknown, path: Path): { readonly id: UserId; readonly name: string } => {
  const object = strict(raw, path, ['id', 'name']);
  return { id: brandId<'UserId'>(member(object, 'id', path, asIdNumber)), name: member(object, 'name', path, asString) };
};

const parseEntry = (raw: unknown, path: Path): HistoryEntry => {
  const object = strict(raw, path, ['id', 'created', 'user', 'source', 'changes']);
  return {
    id: member(object, 'id', path, asIdNumber),
    created: member(object, 'created', path, asString),
    user: member(object, 'user', path, nullable(parseUser)),
    source: member(object, 'source', path, parseSource),
    changes: member(object, 'changes', path, list(parseChange)),
  };
};

export const parseItemHistory: Parser<readonly HistoryEntry[]> = (raw, path = []) =>
  member(strict(raw, path, ['entries']), 'entries', path, list(parseEntry));

export const loadItemHistory = async (item: ItemId): Promise<readonly HistoryEntry[]> =>
  parseItemHistory(await api.requestJson(`/api/romeka/items/${String(item)}/history`));
