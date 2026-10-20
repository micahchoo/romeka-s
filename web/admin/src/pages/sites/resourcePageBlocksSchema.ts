/**
 * The resource page block schema format and its parser: the shape of
 * `src/generated/resourcePageBlocks.json`, which
 * `crates/omeka-site/src/resource_page_schema.rs` generates from the registered
 * layouts and a Rust test keeps in step.
 *
 * The JSON arrives from outside the SPA, so it is `unknown` until
 * [`parseResourcePageBlocksRegistry`] narrows it. A `labelKey` outside the
 * catalogue fails the parse, the way the theme settings registry does.
 *
 * The two pure helpers beside the parser are the storage boundary's other half:
 * [`resourcePageBlockDrafts`] reads a site's stored `resource_page_blocks` map
 * (or the theme's default set when no map stores one), and
 * [`resourcePageBlocksWire`] writes the map back.
 */
import { ParseError, isRecord, parseListOf } from '../../api/parse';
import type { JsonValue, Path } from '../../api/types';
import type { KeyOf } from '../../i18n';
import { sites } from '../../i18n/namespaces/sites';

/** A message id of the `sites` namespace, the table these keys are read from. */
type MessageKey = KeyOf<typeof sites>;

const isMessageKey = sites.has;

const parseString = (value: unknown, path: Path): string => {
  if (typeof value !== 'string') {
    throw new ParseError(path, 'string');
  }
  return value;
};

const parseMessageKey = (value: unknown, path: Path): MessageKey => {
  const key = parseString(value, path);
  if (!isMessageKey(key)) {
    throw new ParseError(path, `a catalogue key, got ${key}`);
  }
  return key;
};

const parseStringList = (value: unknown, path: Path): readonly string[] => {
  if (!Array.isArray(value) || !value.every((item) => typeof item === 'string')) {
    throw new ParseError(path, 'a list of strings');
  }
  return value;
};

/** One registered layout as the admin reads it. */
export interface ResourcePageBlockSpec {
  /** The registered name the stored map carries. */
  readonly name: string;
  /** The catalogue key the editor labels the layout with. */
  readonly labelKey: MessageKey;
  /** The catalogue key of the one line that says what a reader sees. */
  readonly helpKey: MessageKey;
}

/** One resource page: its stored name, its default set and its layouts. */
export interface ResourcePageResourceSpec {
  /** The resource name the stored map uses (`items`, `item_sets`, `media`). */
  readonly key: string;
  /** The catalogue key for the resource's label. */
  readonly labelKey: MessageKey;
  /** The bundled theme's default block names, in render order. */
  readonly default: readonly string[];
  /** The registered layouts compatible with this resource. */
  readonly blocks: readonly ResourcePageBlockSpec[];
}

/** The whole generated registry. */
export interface ResourcePageBlocksRegistry {
  readonly resources: readonly ResourcePageResourceSpec[];
}

const parseBlock = (value: unknown, path: Path): ResourcePageBlockSpec => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    name: parseString(value.name, [...path, 'name']),
    labelKey: parseMessageKey(value.labelKey, [...path, 'labelKey']),
    helpKey: parseMessageKey(value.helpKey, [...path, 'helpKey']),
  };
};

const parseResource = (value: unknown, path: Path): ResourcePageResourceSpec => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    key: parseString(value.key, [...path, 'key']),
    labelKey: parseMessageKey(value.labelKey, [...path, 'labelKey']),
    default: parseStringList(value.default, [...path, 'default']),
    blocks: parseListOf(parseBlock)(value.blocks, [...path, 'blocks']),
  };
};

/** Narrows the generated `resourcePageBlocks.json` into the typed registry. */
export const parseResourcePageBlocksRegistry: (raw: unknown) => ResourcePageBlocksRegistry = (
  raw,
) => {
  if (!isRecord(raw)) {
    throw new ParseError([], 'an object');
  }
  return { resources: parseListOf(parseResource)(raw.resources, ['resources']) };
};

/** The ordered block names one resource holds in the editor. */
export type ResourcePageBlocksDraft = Readonly<Record<string, readonly string[]>>;

/** The `[key].main` list a stored map holds for one resource, `null` when the
 *  map does not name it. */
const storedNames = (stored: Readonly<Record<string, unknown>>, key: string): string[] | null => {
  const resource = stored[key];
  if (!isRecord(resource)) {
    return null;
  }
  const main = resource['main'];
  if (!Array.isArray(main)) {
    return [];
  }
  return main.filter((name): name is string => typeof name === 'string');
};

/**
 * Every resource's draft. A stored map is the administrator's whole answer: a
 * resource it names is read as stored (even as an empty list), and a resource it
 * omits is empty. When no map stores one at all, each resource starts from the
 * theme's default set, which is what the site renders until an administrator
 * edits it.
 */
export const resourcePageBlockDrafts = (
  registry: ResourcePageBlocksRegistry,
  stored: Readonly<Record<string, unknown>>,
): ResourcePageBlocksDraft => {
  const storedBlocks = stored['resource_page_blocks'];
  const override = isRecord(storedBlocks) ? storedBlocks : null;
  const drafts: Record<string, readonly string[]> = {};
  for (const resource of registry.resources) {
    drafts[resource.key] =
      override === null ? [...resource.default] : (storedNames(override, resource.key) ?? []);
  }
  return drafts;
};

/** The stored `resource_page_blocks` map, from the drafts. Every resource is
 *  named, so a write is the administrator's whole answer. */
export const resourcePageBlocksWire = (
  registry: ResourcePageBlocksRegistry,
  drafts: ResourcePageBlocksDraft,
): Readonly<Record<string, JsonValue>> => {
  const blocks: Record<string, JsonValue> = {};
  for (const resource of registry.resources) {
    blocks[resource.key] = { main: [...(drafts[resource.key] ?? [])] };
  }
  return blocks;
};
