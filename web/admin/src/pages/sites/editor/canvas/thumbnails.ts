/**
 * The small image a canvas card shows for a block that names one.
 *
 * The draft holds only ids: an attachment's item and media, an asset field's
 * asset id. Which field names an image comes from the layout's schema, so a new
 * layout with an `attachments`, `assetRef` or `mediaRef` field gets a thumbnail with no
 * code here. The canvas resolves every card's id in one read per kind, never
 * one read per card.
 */
import { api } from '../../../../api/client';
import type { JsonValue } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import type { BlockField, BlockRegistry } from '../../blockSchema';
import type { BlockDraft } from '../draft';
import { carrierOf, romkaFor } from '../romeka';
import { schemaFor } from '../schema';
import { dataObject, isJsonArray, isJsonObject } from '../values';
import { isShown } from '../fields/visibility';

export type ImageKind = 'media' | 'item' | 'asset';

export interface ImageRef {
  readonly kind: ImageKind;
  readonly id: number;
}

const idOf = (value: JsonValue | undefined): number | null => {
  const id = typeof value === 'string' ? Number(value) : value;
  return typeof id === 'number' && Number.isInteger(id) && id > 0 ? id : null;
};

/**
 * The first image the fields hold: an asset or a media reference whose control
 * shows, so a hidden choice left behind by a switch is not the card's image.
 * Repeated entries are looked inside too.
 */
const firstImage = (
  fields: readonly BlockField[],
  values: Readonly<Record<string, JsonValue>>,
): ImageRef | null => {
  for (const field of fields) {
    if (!isShown(field, values, fields)) {
      continue;
    }
    const value = values[field.key];
    if (field.type === 'assetRef' || field.type === 'mediaRef') {
      const id = idOf(value);
      if (id !== null) {
        return { kind: field.type === 'assetRef' ? 'asset' : 'media', id };
      }
    } else if (field.type === 'repeat' && value !== undefined && isJsonArray(value)) {
      for (const entry of value) {
        const found = isJsonObject(entry) ? firstImage(field.fields, entry) : null;
        if (found !== null) {
          return found;
        }
      }
    }
  }
  return null;
};

/** The image a block's card shows: its first attachment, else its first image field. */
export const blockImage = (registry: BlockRegistry, block: BlockDraft): ImageRef | null => {
  const romka = romkaFor(registry, block);
  if (romka !== undefined) {
    return firstImage(romka.fields, carrierOf(block)?.settings ?? {});
  }
  const schema = schemaFor(registry, block.layout);
  if (schema === undefined) {
    return null;
  }
  const attachments = schema.fields.find((field) => field.type === 'attachments');
  const first = block.attachments[0];
  if (attachments !== undefined && first !== undefined) {
    if (attachments.media && first.media !== null) {
      return { kind: 'media', id: first.media };
    }
    if (first.item !== null) {
      return { kind: 'item', id: first.item };
    }
  }
  return firstImage(schema.fields, dataObject(block));
};

export const imageKey = (ref: ImageRef): string => `${ref.kind}:${String(ref.id)}`;

const idsOf = (refs: readonly ImageRef[], kind: ImageKind): readonly number[] =>
  [...new Set(refs.filter((ref) => ref.kind === kind).map((ref) => ref.id))].sort((a, b) => a - b);

/** One `id=a,b,c` read per kind that has ids; a kind with none reads nothing. */
const readUrls = async (
  kind: ImageKind,
  ids: readonly number[],
): Promise<readonly (readonly [string, string])[]> => {
  if (ids.length === 0) {
    return [];
  }
  const query = { id: ids.join(','), perPage: ids.length };
  const pairs: (readonly [string, string | undefined])[] =
    kind === 'asset'
      ? (await api.listAssets(query)).items.map((asset) => [`asset:${String(asset.id)}`, asset.assetUrl])
      : kind === 'media'
        ? (await api.listMedia(query)).items.map((media) => [
            `media:${String(media.id)}`,
            media.thumbnailDisplayUrls.square,
          ])
        : (await api.listItems(query)).items.map((item) => [
            `item:${String(item.id)}`,
            item.thumbnailDisplayUrls.square,
          ]);
  return pairs.filter((pair): pair is readonly [string, string] => pair[1] !== undefined);
};

/** The thumbnail URL of every image the blocks name, keyed by `imageKey`. */
export const useThumbnails = (
  registry: BlockRegistry,
  blocks: readonly BlockDraft[],
): ReadonlyMap<string, string> => {
  const refs = blocks
    .map((block) => blockImage(registry, block))
    .filter((ref): ref is ImageRef => ref !== null);
  const kinds: readonly ImageKind[] = ['media', 'item', 'asset'];
  const wanted = kinds.map((kind) => idsOf(refs, kind));
  const signature = wanted.map((ids) => ids.join(',')).join('|');
  const state = useAsync(
    async () =>
      new Map((await Promise.all(kinds.map((kind, at) => readUrls(kind, wanted[at] ?? [])))).flat()),
    [signature],
  );
  return state.status === 'ready' ? state.data : new Map();
};
