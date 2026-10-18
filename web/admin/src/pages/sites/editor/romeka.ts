/**
 * The Romeka carrier, read from a draft block.
 *
 * A Romeka block is stored as an upstream block whose `o:layout_data.romeka`
 * holds `{block, v, settings, plain_hash}`; `docs/compat.md` records the rule.
 * The editor reads that member to label the block and render its settings form.
 * The hash that marks a carrier edited outside Romeka lives in
 * [`romekaHash`](./romekaHash.ts), apart from this module, so the shared shell
 * chunk does not carry it.
 */
import type { JsonValue } from '../../../api/types';
import type { BlockRegistry, RomkaBlockSchema } from '../blockSchema';
import type { LayoutData } from './draft';
import { isJsonObject, layoutObject } from './values';

/** The `o:layout_data.romeka` member of a stored block. */
export interface RomkaData {
  readonly block: string;
  readonly v: number;
  readonly settings: Readonly<Record<string, JsonValue>>;
  readonly plainHash: string | null;
}

const text = (value: JsonValue | undefined): string =>
  typeof value === 'string' ? value : '';

/** The carrier a block's layout data holds, `null` when it holds none. */
export const carrierOf = (block: {
  readonly layoutData: LayoutData | null;
}): RomkaData | null => {
  if (block.layoutData === null) {
    return null;
  }
  const romeka = layoutObject(block.layoutData).romeka;
  if (romeka === undefined || !isJsonObject(romeka)) {
    return null;
  }
  const name = text(romeka.block);
  if (name === '') {
    return null;
  }
  const version = typeof romeka.v === 'number' ? romeka.v : 1;
  const settingsValue = romeka.settings;
  const settings =
    settingsValue !== undefined && isJsonObject(settingsValue) ? settingsValue : {};
  const plain = text(romeka.plain_hash);
  return {
    block: name,
    v: version,
    settings,
    plainHash: plain === '' ? null : plain,
  };
};

/** The registered Romeka block a draft block carries, `undefined` when none. */
export const romkaFor = (
  registry: BlockRegistry,
  block: { readonly layoutData: LayoutData | null },
): RomkaBlockSchema | undefined => {
  const carrier = carrierOf(block);
  if (carrier === null) {
    return undefined;
  }
  return registry.romeka.find((entry) => entry.name === carrier.block);
};

/** The settings record a Romka form reads. */
export const settingsOf = (
  block: { readonly layoutData: LayoutData | null },
): Readonly<Record<string, JsonValue>> => carrierOf(block)?.settings ?? {};

/** The `o:layout_data` value for a new carrier: the block name, the carrier
 * version and the settings, under the `romeka` member the write and the
 * renderer read. */
export const newCarrier = (
  name: string,
  settings: Readonly<Record<string, JsonValue>>,
  version: number,
): Readonly<Record<string, JsonValue>> => ({
  romeka: {
    block: name,
    v: version,
    settings,
  },
});

/** The same layout data with `romeka.plain_hash` removed, so the next save
 * regenerates the plain rendering from the Romeka version. */
export const withoutPlainHash = (layoutData: LayoutData): LayoutData => {
  const outer = layoutObject(layoutData);
  const romeka = outer.romeka;
  if (romeka === undefined || !isJsonObject(romeka)) {
    return layoutData;
  }
  const rest: Record<string, JsonValue> = {};
  for (const [key, value] of Object.entries(romeka)) {
    if (key !== 'plain_hash') {
      rest[key] = value;
    }
  }
  return { ...outer, romeka: rest };
};

/** The same layout data with the carrier's settings replaced. */
export const withSettings = (
  layoutData: LayoutData,
  settings: Readonly<Record<string, JsonValue>>,
): LayoutData => {
  const outer = layoutObject(layoutData);
  const romeka = outer.romeka;
  if (romeka === undefined || !isJsonObject(romeka)) {
    return layoutData;
  }
  return { ...outer, romeka: { ...romeka, settings } };
};
