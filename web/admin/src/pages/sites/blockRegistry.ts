/**
 * The block registry the admin's editor reads: the layout names and their
 * labels, generated from `crates/omeka-site/src/blocks` into
 * `src/generated/blocks.json` by `blocks::schema`, and narrowed by
 * [`parseBlockRegistry`](./blockSchema). The hand-mirrored `layouts.ts` this
 * replaces is gone; a Rust test fails when the file drifts from the registry.
 */
import raw from '../../generated/blocks.json';
import type { MessageKey } from '../../i18n';
import { parseBlockRegistry } from './blockSchema';
import type { BlockRegistry } from './blockSchema';

/** The generated registry, parsed at this boundary. */
export const BLOCK_REGISTRY: BlockRegistry = parseBlockRegistry(raw);

/** One option of the block layout select. */
export interface BlockLayoutOption {
  readonly name: string;
  readonly labelKey: MessageKey;
}

/** Every registered layout, in the registry's order. */
export const BLOCK_LAYOUTS: readonly BlockLayoutOption[] = BLOCK_REGISTRY.layouts.map((layout) => ({
  name: layout.name,
  labelKey: layout.label,
}));

/** Every registered Romeka block, in the registry's order. */
export const ROMKA_BLOCKS: readonly BlockLayoutOption[] = BLOCK_REGISTRY.romeka.map((block) => ({
  name: block.name,
  labelKey: block.label,
}));

/**
 * What the insert picker offers: the upstream layouts and the Romeka blocks.
 * Choosing a Romeka block stores an upstream carrier, which the draft model
 * builds from `block.carrier`.
 */
export const BLOCK_PICKER_OPTIONS: readonly BlockLayoutOption[] = [
  ...BLOCK_LAYOUTS,
  ...ROMKA_BLOCKS,
];

/**
 * The options for one block, keeping a stored layout the registry does not
 * register as its own entry so an unknown layout is never silently switched.
 */
export const blockLayoutOptions = (current: string): readonly BlockLayoutOption[] =>
  BLOCK_LAYOUTS.some((option) => option.name === current)
    ? BLOCK_LAYOUTS
    : [...BLOCK_LAYOUTS, { name: current, labelKey: 'layout.unknown' }];
