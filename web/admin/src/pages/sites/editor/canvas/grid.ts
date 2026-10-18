/**
 * The page grid as the canvas reads it: the column count, the two gaps and a
 * card's placement. The rules mirror the public renderer
 * (`crates/omeka-site/src/blocks/layout.rs`): a position outside `1..columns`
 * is `auto`, a span outside that range is the column count, and an unset gap is
 * 10 pixels.
 */
import type { JsonValue } from '../../../../api/types';
import type { BlockDraft, LayoutData } from '../draft';
import { layoutDataObject, layoutObject } from '../values';

const integerValue = (value: JsonValue | undefined): number | null => {
  if (typeof value === 'number') {
    return Number.isInteger(value) ? value : null;
  }
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (trimmed === '') {
      return null;
    }
    const parsed = Number(trimmed);
    return Number.isInteger(parsed) ? parsed : null;
  }
  return null;
};

const nonNegative = (value: JsonValue | undefined): number | null => {
  const parsed = integerValue(value);
  return parsed !== null && parsed >= 0 ? parsed : null;
};

/** `grid_columns`, or `0` when the page does not set a count. */
export const pageColumns = (layoutData: LayoutData): number =>
  nonNegative(layoutObject(layoutData).grid_columns) ?? 0;

/** A grid gap in pixels, defaulting to the renderer's 10. */
export const gridGap = (
  layoutData: LayoutData,
  key: 'grid_column_gap' | 'grid_row_gap',
): number => nonNegative(layoutObject(layoutData)[key]) ?? 10;

/** Where one card sits in the grid, with the renderer's fallbacks applied. */
export interface Placement {
  /** A one-based column, or `auto` when the stored value is absent or invalid. */
  readonly position: number | 'auto';
  readonly span: number;
}

const positionOf = (value: JsonValue | undefined, columns: number): number | 'auto' => {
  if (value === 'auto') {
    return 'auto';
  }
  const parsed = integerValue(value);
  return parsed !== null && parsed >= 1 && parsed <= columns ? parsed : 'auto';
};

const spanOf = (value: JsonValue | undefined, columns: number): number => {
  const parsed = integerValue(value);
  return parsed !== null && parsed >= 1 && parsed <= columns ? parsed : columns;
};

export const placementOf = (block: BlockDraft, columns: number): Placement => {
  const data = layoutDataObject(block);
  return {
    position: positionOf(data.grid_column_position, columns),
    span: spanOf(data.grid_column_span, columns),
  };
};
