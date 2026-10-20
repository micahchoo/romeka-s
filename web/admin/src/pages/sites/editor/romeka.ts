/**
 * The Romeka carrier, `o:layout_data.romeka = {block, v, settings,
 * plain_hash}`: the one place the editor reads or writes it
 * (`docs/compat.md` records the rule).
 *
 * Two kinds of block keep one. A Romeka block is stored as an upstream block
 * whose carrier names the Romeka block, and its settings are that block's own
 * data. An upstream block keeps its Romeka settings (a section, shared
 * styling, a viewer height) in a carrier that names its own layout.
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

type JsonRecord = Readonly<Record<string, JsonValue>>;

/** The carrier a layout-data record holds, `null` when it holds none: a
 * carrier names its block. */
const carrierIn = (record: JsonRecord): RomkaData | null => {
  const romeka = record.romeka;
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

/** The carrier a block's layout data holds, `null` when it holds none. */
export const carrierOf = (block: {
  readonly layoutData: LayoutData | null;
}): RomkaData | null =>
  block.layoutData === null ? null : carrierIn(layoutObject(block.layoutData));

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

/** The setting at a dotted path such as `section.layout`, `undefined` when
 * the record holds no carrier or the carrier holds no such setting. */
export const readSetting = (record: JsonRecord, path: string): JsonValue | undefined => {
  let current: JsonValue | undefined = carrierIn(record)?.settings;
  for (const step of path.split('.')) {
    if (current === undefined || !isJsonObject(current)) {
      return undefined;
    }
    current = current[step];
  }
  return current;
};

/** A setting value that says "not set" on an upstream block. */
const isUnset = (value: JsonValue): boolean => value === null || value === '' || value === false;

const without = (record: JsonRecord, key: string): JsonRecord =>
  Object.fromEntries(Object.entries(record).filter(([name]) => name !== key));

/** `record` with `value` at the path `steps`. With `dropUnset`, an unset value
 * removes its member, and an object it leaves empty goes with it. */
const writeNested = (
  record: JsonRecord,
  steps: readonly string[],
  value: JsonValue,
  dropUnset: boolean,
): JsonRecord => {
  const [step, ...rest] = steps;
  if (step === undefined) {
    return record;
  }
  if (rest.length === 0) {
    return dropUnset && isUnset(value) ? without(record, step) : { ...record, [step]: value };
  }
  const child = record[step];
  const written = writeNested(
    child !== undefined && isJsonObject(child) ? child : {},
    rest,
    value,
    dropUnset,
  );
  return dropUnset && Object.keys(written).length === 0
    ? without(record, step)
    : { ...record, [step]: written };
};

/** The block a settings write belongs to. */
export interface CarrierOwner {
  /** The block's stored `layout`, which a carrier of its own settings names. */
  readonly layout: string;
  /** The carrier version a new carrier records. */
  readonly version: number;
}

/**
 * `record` with each of `writes` (a dotted setting path to its value) written
 * into the carrier's settings.
 *
 * A Romeka block's carrier keeps every value as given, as `o:data` does. An
 * upstream block's carrier holds only what Romeka adds: an unset value is
 * removed, the first write creates the carrier naming the block's own layout,
 * and a carrier left with no settings and nothing else is removed, so upstream
 * sees no trace of a cleared setting.
 */
export const writeSettings = (
  record: JsonRecord,
  writes: JsonRecord,
  owner: CarrierOwner,
): JsonRecord => {
  const carrier = carrierIn(record);
  const stored = record.romeka;
  const raw = stored !== undefined && isJsonObject(stored) ? stored : {};
  const dropUnset = carrier === null || carrier.block === owner.layout;
  const settings = Object.entries(writes).reduce<JsonRecord>(
    (current, [path, value]) => writeNested(current, path.split('.'), value, dropUnset),
    carrier?.settings ?? {},
  );
  const bare = Object.keys(raw).every((key) => key === 'block' || key === 'v' || key === 'settings');
  if (dropUnset && Object.keys(settings).length === 0 && bare) {
    return without(record, 'romeka');
  }
  return {
    ...record,
    romeka:
      carrier === null
        ? { v: owner.version, ...raw, block: owner.layout, settings }
        : { ...raw, settings },
  };
};
