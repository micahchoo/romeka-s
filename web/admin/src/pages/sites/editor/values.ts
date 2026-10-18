/**
 * The value helpers behind the draft: the two `o:data` shapes, their object
 * views, and the equality that decides whether an edit changed anything.
 */
import type { JsonValue } from '../../../api/types';
import type {
  BlockAttachment,
  BlockData,
  BlockDraft,
  LayoutData,
  PageContent,
} from './draft';

export const isJsonObject = (
  value: JsonValue,
): value is Readonly<Record<string, JsonValue>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export const isJsonArray = (value: JsonValue): value is readonly JsonValue[] =>
  Array.isArray(value);

/** A stored `o:data` or `o:layout_data`, keeping an array as an array. */
export const readData = (value: JsonValue): BlockData => {
  if (isJsonArray(value)) {
    return value;
  }
  return isJsonObject(value) ? value : {};
};

export const readLayoutData = (value: JsonValue | null): LayoutData | null =>
  value === null ? null : readData(value);

/** The object view of a block's data, for a form or a validator. */
export const dataObject = (block: BlockDraft): Readonly<Record<string, JsonValue>> =>
  isJsonObject(block.data) ? block.data : {};

/** The object view of a layout-data value; an empty array has no members. */
export const layoutObject = (value: LayoutData): Readonly<Record<string, JsonValue>> =>
  isJsonObject(value) ? value : {};

/** The object view of a block's layout data, for a form or the grid canvas. */
export const layoutDataObject = (block: BlockDraft): Readonly<Record<string, JsonValue>> =>
  block.layoutData === null ? {} : layoutObject(block.layoutData);

export const jsonEqual = (left: JsonValue, right: JsonValue): boolean => {
  if (left === right) {
    return true;
  }
  if (isJsonArray(left) && isJsonArray(right)) {
    return (
      left.length === right.length &&
      left.every((item, index) => {
        const other = right[index];
        return other !== undefined && jsonEqual(item, other);
      })
    );
  }
  if (isJsonObject(left) && isJsonObject(right)) {
    const leftKeys = Object.keys(left);
    const rightKeys = Object.keys(right);
    return (
      leftKeys.length === rightKeys.length &&
      leftKeys.every((key) => {
        const mine = left[key];
        const theirs = right[key];
        return mine !== undefined && theirs !== undefined && jsonEqual(mine, theirs);
      })
    );
  }
  return false;
};

const attachmentsEqual = (
  left: readonly BlockAttachment[],
  right: readonly BlockAttachment[],
): boolean =>
  left.length === right.length &&
  left.every((attachment, index) => {
    const other = right[index];
    return (
      other !== undefined &&
      attachment.item === other.item &&
      attachment.media === other.media &&
      attachment.caption === other.caption
    );
  });

const blockEquals = (left: BlockDraft, right: BlockDraft): boolean =>
  left.key === right.key &&
  left.layout === right.layout &&
  jsonEqual(left.data, right.data) &&
  (left.layoutData === null
    ? right.layoutData === null
    : right.layoutData !== null && jsonEqual(left.layoutData, right.layoutData)) &&
  attachmentsEqual(left.attachments, right.attachments);

const blocksEqual = (left: readonly BlockDraft[], right: readonly BlockDraft[]): boolean =>
  left.length === right.length &&
  left.every((block, index) => {
    const other = right[index];
    return other !== undefined && blockEquals(block, other);
  });

export const contentEquals = (left: PageContent, right: PageContent): boolean =>
  left.title === right.title &&
  left.slug === right.slug &&
  left.isPublic === right.isPublic &&
  left.layout === right.layout &&
  jsonEqual(left.layoutData, right.layoutData) &&
  blocksEqual(left.blocks, right.blocks);
