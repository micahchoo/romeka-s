/**
 * Whether a block is written: the words a visitor reads are in it.
 *
 * A block's schema marks at most one field `primary`: the hero's heading, an
 * HTML block's text, a quote, a list's pages. While that field is empty the
 * block is not written, and the server leaves it out for visitors
 * (`crates/omeka-site/src/blocks/written.rs`). This module is the one place
 * the admin decides it, for the editor's block list (live on the draft) and
 * for the site hub's page list (on the stored pages), so the two counts
 * cannot differ. Blank HTML is `blankHtml.ts`'s rule, which the html field's
 * hint reads too.
 *
 * It also finds the text earlier templates laid down
 * (`former-template-text.json`) on a page saved before the templates left
 * their fields empty, so the editor can offer to clear it.
 */
import type { JsonValue, SitePageBlock } from '../../../api/types';
import formerText from '../../../generated/former-template-text.json';
import { BLOCK_REGISTRY } from '../blockRegistry';
import { isBlankHtml } from './blankHtml';
import type { BlockField, BlockRegistry } from '../blockSchema';
import type { BlockKey, DraftAction, LayoutData } from './draft';
import { romekaKey } from './layoutPaths';
import { carrierOf, romkaFor } from './romeka';
import { romkaState } from './romekaHash';
import { schemaFor } from './schema';
import { isJsonArray, isJsonObject, readLayoutData } from './values';
import type { EditorMessageKey } from './messages';

/** A block as the draft and the stored page both hold it. */
export interface BlockContent {
  readonly layout: string;
  readonly data: JsonValue;
  readonly layoutData: LayoutData | null;
}

type JsonRecord = Readonly<Record<string, JsonValue>>;

const isBlank = (field: BlockField, value: JsonValue | undefined): boolean => {
  if (value === undefined || value === null) {
    return true;
  }
  if (typeof value !== 'string') {
    return false;
  }
  if (field.type === 'html') {
    return isBlankHtml(value);
  }
  if (field.type === 'pageList') {
    if (value.trim() === '') {
      return true;
    }
    try {
      const nodes: unknown = JSON.parse(value);
      return Array.isArray(nodes) && nodes.length === 0;
    } catch {
      return false;
    }
  }
  return value.trim() === '';
};

const primaryOf = (fields: readonly BlockField[]): BlockField | undefined =>
  fields.find((field) => field.primary);

const recordOf = (value: JsonValue): JsonRecord => (isJsonObject(value) ? value : {});

/**
 * Where the block keeps its words: a Romeka block's settings, unless its
 * stored HTML was edited outside Romeka (the server then renders that HTML),
 * else its own `o:data`.
 */
const wordsOf = (
  registry: BlockRegistry,
  block: BlockContent,
): { readonly fields: readonly BlockField[]; readonly record: JsonRecord; readonly carrier: boolean } => {
  const romka = romkaFor(registry, block);
  if (romka !== undefined && romkaState(registry, block) !== 'edited') {
    return { fields: romka.fields, record: carrierOf(block)?.settings ?? {}, carrier: true };
  }
  const layout = schemaFor(registry, block.layout);
  return { fields: layout?.fields ?? [], record: recordOf(block.data), carrier: false };
};

/** The block's empty primary field, `null` when the block is written or names none. */
export const unwrittenField = (registry: BlockRegistry, block: BlockContent): BlockField | null => {
  const { fields, record } = wordsOf(registry, block);
  const field = primaryOf(fields);
  return field !== undefined && isBlank(field, record[field.key]) ? field : null;
};

/** How many of the blocks are not written. */
export const unwrittenCount = (registry: BlockRegistry, blocks: readonly BlockContent[]): number =>
  blocks.filter((block) => unwrittenField(registry, block) !== null).length;

/** How many of a stored page's blocks are not written, by the generated registry. */
export const storedUnwrittenCount = (blocks: readonly SitePageBlock[]): number =>
  unwrittenCount(
    BLOCK_REGISTRY,
    blocks.map((block) => ({
      layout: block.layout,
      data: block.data,
      layoutData: readLayoutData(block.layoutData),
    })),
  );

/** Whitespace runs as single spaces, trimmed: the form two texts are compared in. */
export const foldSpace = (text: string): string => text.replace(/\s+/g, ' ').trim();

const parseFormerText = (value: unknown): ReadonlySet<string> =>
  new Set(
    (Array.isArray(value) ? value : [])
      .filter((entry): entry is string => typeof entry === 'string')
      .map(foldSpace),
  );

/** The text earlier templates laid down, folded. */
export const FORMER_TEXT: ReadonlySet<string> = parseFormerText(formerText);

/** The keys of the block's text fields whose value is former template text. */
export const templateTextKeys = (
  registry: BlockRegistry,
  block: BlockContent,
  former: ReadonlySet<string> = FORMER_TEXT,
): readonly string[] => {
  const { record } = wordsOf(registry, block);
  return Object.keys(record).filter((key) => {
    const value = record[key];
    return typeof value === 'string' && value !== '' && former.has(foldSpace(value));
  });
};

/**
 * The one edit that empties the block's former template text, `null` when it
 * holds none. It is an ordinary draft edit: undoable, and saved like any other.
 */
export const clearTemplateText = (
  registry: BlockRegistry,
  block: BlockContent & { readonly key: BlockKey },
  former: ReadonlySet<string> = FORMER_TEXT,
): DraftAction | null => {
  const keys = templateTextKeys(registry, block, former);
  if (keys.length === 0) {
    return null;
  }
  if (wordsOf(registry, block).carrier) {
    return {
      type: 'setLayoutDataValues',
      blockKey: block.key,
      values: Object.fromEntries(keys.map((key) => [romekaKey(key), ''])),
    };
  }
  const data = isJsonArray(block.data) ? {} : recordOf(block.data);
  return {
    type: 'setBlockData',
    blockKey: block.key,
    data: { ...data, ...Object.fromEntries(keys.map((key) => [key, ''])) },
  };
};

/**
 * The hint text of every primary field, keyed by its hint key, in the active
 * locale. The preview request carries it, because the server does not know
 * the admin's locale and the frame's hint must read as the field's does.
 */
export const previewHints = (
  registry: BlockRegistry,
  t: (key: EditorMessageKey) => string,
): Readonly<Record<string, string>> => {
  const hints: Record<string, string> = {};
  for (const entry of [...registry.layouts, ...registry.romeka]) {
    const field = primaryOf(entry.fields);
    if (
      field !== undefined &&
      (field.type === 'text' || field.type === 'html' || field.type === 'pageList') &&
      field.placeholder !== null
    ) {
      hints[field.placeholder] = t(field.placeholder);
    }
  }
  return hints;
};
