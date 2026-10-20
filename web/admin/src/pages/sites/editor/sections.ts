/**
 * A block group's section: the layout under `o:layout_data.romeka.section`
 * and the slots it gives the group's blocks.
 *
 * A section's blocks are its slots, in order: a 40/60 split's first block is
 * the narrow column and its second the wide one, and a row of three cards
 * takes the next three blocks. The public stylesheet lays them out
 * (`crates/omeka-site/src/theme/style.css`, "Sections"); the shapes here draw
 * the same columns in the Outline and in the layout picker's diagrams.
 */
import type { JsonValue } from '../../../api/types';
import type { LayoutData } from './draft';
import { GROUP_LAYOUT } from './groups';
import { readPath } from './layoutPaths';
import { layoutObject } from './values';

/** The layout-data path of a group's section layout. */
export const SECTION_LAYOUT_PATH = 'romeka.section.layout';

/** How a layout draws: its column widths as fractions, and whether its slots are cards. */
export interface SectionShape {
  readonly columns: readonly number[];
  readonly cards: boolean;
  /** Whether the section spans the whole window. */
  readonly bleed: boolean;
}

const shape = (columns: readonly number[], cards = false, bleed = false): SectionShape => ({
  columns,
  cards,
  bleed,
});

/** Every layout the renderer knows (`style.rs::SectionLayout`), by stored name. */
export const SECTION_SHAPES: ReadonlyMap<string, SectionShape> = new Map([
  ['stack', shape([1])],
  ['split-50-50', shape([1, 1])],
  ['split-40-60', shape([2, 3])],
  ['split-60-40', shape([3, 2])],
  ['cards-2', shape([1, 1], true)],
  ['cards-3', shape([1, 1, 1], true)],
  ['cards-4', shape([1, 1, 1, 1], true)],
  ['band', shape([1], false, true)],
]);

/** The CSS grid columns a shape draws. */
export const gridColumns = (section: SectionShape): string =>
  section.columns.map((fraction) => `minmax(0, ${String(fraction)}fr)`).join(' ');

/** A group's section: its stored layout name and shape. */
export interface SectionOf {
  readonly layout: string;
  readonly shape: SectionShape;
}

/** The section a block group has, `null` for a block that is no group or a group without a known layout. */
export const sectionOf = (block: {
  readonly layout: string;
  readonly layoutData: LayoutData | null;
}): SectionOf | null => {
  if (block.layout !== GROUP_LAYOUT || block.layoutData === null) {
    return null;
  }
  const layout: JsonValue | undefined = readPath(layoutObject(block.layoutData), SECTION_LAYOUT_PATH);
  if (typeof layout !== 'string') {
    return null;
  }
  const found = SECTION_SHAPES.get(layout);
  return found === undefined ? null : { layout, shape: found };
};

/**
 * The empty slots after a section's blocks: the rest of the last row, or one
 * full row when the section holds no block yet.
 */
export const emptySlots = (section: SectionShape, members: number): number => {
  const columns = section.columns.length;
  if (members === 0) {
    return columns;
  }
  const rest = members % columns;
  return rest === 0 ? 0 : columns - rest;
};
