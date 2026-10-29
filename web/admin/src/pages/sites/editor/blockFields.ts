/**
 * The fields the server reads of a stored or drafted block, with their values.
 *
 * A Romeka block's settings, unless its stored HTML was edited outside Romeka
 * (the server then renders that HTML); else its layout's fields, an upstream
 * member read from `o:data` and a `romeka.` key from the carrier's settings.
 * A field whose `when` test hides it is left out: the server reads only what
 * the block's current choices use (`map`'s query only while it lists by query).
 *
 * The Outline's "Lists nothing yet" and the site hub's Site check both read a
 * block through here, so the two never disagree on what a block lists.
 */
import { brandId } from '../../../api/parse';
import type { ItemId, JsonValue, MediaId } from '../../../api/types';
import type { BlockField, BlockRegistry } from '../blockSchema';
import { isQueryResource } from './fields/listReach';
import type { QueryResource } from './fields/listReach';
import { isShown } from './fields/visibility';
import { readPath, settingPath } from './layoutPaths';
import { romkaFor, settingsOf } from './romeka';
import { romkaState } from './romekaHash';
import { schemaFor } from './schema';
import { isJsonArray, isJsonObject, layoutObject } from './values';
import type { BlockContent } from './written';

type JsonRecord = Readonly<Record<string, JsonValue>>;

interface ReadFields {
  readonly fields: readonly BlockField[];
  readonly values: JsonRecord;
}

const layoutValues = (block: BlockContent, fields: readonly BlockField[]): JsonRecord => {
  const values: Record<string, JsonValue> = isJsonObject(block.data) ? { ...block.data } : {};
  const layout = block.layoutData === null ? {} : layoutObject(block.layoutData);
  for (const field of fields) {
    const value = settingPath(field.key) === null ? undefined : readPath(layout, field.key);
    if (value !== undefined) {
      values[field.key] = value;
    }
  }
  return values;
};

/** The block's shown fields and the values they read. */
const readFields = (registry: BlockRegistry, block: BlockContent): ReadFields => {
  const romka = romkaFor(registry, block);
  const shown = (fields: readonly BlockField[], values: JsonRecord): ReadFields => ({
    fields: fields.filter((field) => isShown(field, values, fields)),
    values,
  });
  if (romka !== undefined && romkaState(registry, block) !== 'edited') {
    return shown(romka.fields, settingsOf(block));
  }
  const fields = schemaFor(registry, block.layout)?.fields ?? [];
  return shown(fields, layoutValues(block, fields));
};

const text = (value: JsonValue | undefined): string => (typeof value === 'string' ? value : '');

/** The query a block lists by and the resource it searches, `null` for a block that lists nothing by query. */
export const blockList = (
  registry: BlockRegistry,
  block: BlockContent,
): { readonly resource: QueryResource; readonly query: string } | null => {
  const { fields, values } = readFields(registry, block);
  for (const field of fields) {
    if (field.type === 'itemQuery') {
      const resource = field.resourceKey === null ? 'items' : values[field.resourceKey];
      return {
        resource: isQueryResource(resource) ? resource : 'items',
        query: text(values[field.key]),
      };
    }
  }
  return null;
};

/** One item or media a block shows, and the media's item when the block names it. */
export interface ResourceRef {
  readonly item: ItemId | null;
  readonly media: MediaId | null;
}

/** A positive whole number, as the server's `whole` reads a stored id. */
const wholeId = (value: JsonValue | undefined): number | null => {
  const number =
    typeof value === 'number' ? value : typeof value === 'string' && /^\s*\d+\s*$/.test(value) ? Number(value) : NaN;
  return Number.isInteger(number) && number > 0 ? number : null;
};

const ref = (item: JsonValue | undefined, media: JsonValue | undefined): ResourceRef | null => {
  const itemId = wholeId(item);
  const mediaId = wholeId(media);
  return itemId === null && mediaId === null
    ? null
    : {
        item: itemId === null ? null : brandId<'ItemId'>(itemId),
        media: mediaId === null ? null : brandId<'MediaId'>(mediaId),
      };
};

const listOf = (value: JsonValue | undefined): readonly JsonValue[] => (value !== undefined && isJsonArray(value) ? value : []);

/** The refs one shown field holds. */
const fieldRefs = (
  field: BlockField,
  values: JsonRecord,
  attachments: readonly ResourceRef[],
): readonly (ResourceRef | null)[] => {
  const value = values[field.key];
  switch (field.type) {
    case 'attachments':
      return field.media ? attachments : attachments.map((entry) => ({ item: entry.item, media: null }));
    case 'itemRef':
      return [ref(value, undefined)];
    case 'itemList':
      return listOf(value).map((entry) => ref(entry, undefined));
    case 'itemMedia':
      return listOf(value).map((entry) => (isJsonObject(entry) ? ref(entry.item, entry.media) : null));
    case 'mediaRef':
      return [ref(undefined, value)];
    default:
      return [];
  }
};

/**
 * Every item and media the block shows by id: its attachments where its form
 * has an attachments field, and its picked items and media. An asset has no
 * visibility, and a list's items come from its query, so neither is a ref.
 */
export const blockRefs = (
  registry: BlockRegistry,
  block: BlockContent,
  attachments: readonly ResourceRef[],
): readonly ResourceRef[] => {
  const { fields, values } = readFields(registry, block);
  return fields
    .flatMap((field) => fieldRefs(field, values, attachments))
    .filter((entry): entry is ResourceRef => entry !== null);
};
