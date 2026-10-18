/**
 * The canonical content hash of a block's stored html.
 *
 * An upstream admin save rewrites a stored html block through HTMLPurifier, so
 * a hash of the raw bytes changes on every save even when no content did. This
 * hashes a canonical content form instead: the ordered visible text runs with
 * whitespace collapsed, each image's `src` and `alt`, and each link's `href`
 * with the text it wraps. Formatting, attribute order, whitespace between
 * elements and `rel`/`target` are dropped. It matches
 * `crates/omeka-site/src/blocks/canonical.rs`.
 *
 * It lives apart from `romeka.ts` because only the canvas marks a carrier: a
 * module the shared shell chunk also pulls in must stay small.
 */
import type { JsonValue } from '../../../api/types';
import type { BlockRegistry } from '../blockSchema';
import type { LayoutData } from './draft';
import { isJsonObject } from './values';
import type { RomkaData } from './romeka';
import { carrierOf, romkaFor } from './romeka';

const OFFSET_BASIS = 0xcbf29ce484222325n;
const PRIME = 0x100000001b3n;
const MASK = 0xffffffffffffffffn;

/** FNV-1a 64 over a string's UTF-8 bytes, as 16 lowercase hex digits. */
const fnv1a = (text: string): string => {
  let hash = OFFSET_BASIS;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = (hash * PRIME) & MASK;
  }
  return hash.toString(16).padStart(16, '0');
};

/** The raw-byte hash carrier version 1 stored. */
export const legacyPlainHash = (text: string): string => fnv1a(text);

const ATOM = '\u001f';
const FIELD = '\u001e';

/** The characters collapsed to one space, matching `canonical.rs`. */
const COLLAPSIBLE = /[\t\n\r\v\f \u00a0]+/g;

const collapse = (text: string): string =>
  text.replace(COLLAPSIBLE, ' ').replace(/^ | $/g, '');

const HIDDEN = new Set(['script', 'style', 'template', 'head', 'title', 'noscript']);

/** Walk a parsed node, appending one atom per visible item, in order. */
const atomsOf = (node: Node, atoms: string[], linked: boolean[]): void => {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = collapse(node.nodeValue ?? '');
    if (text !== '') {
      atoms.push(`t${FIELD}${text}`);
    }
    return;
  }
  if (!(node instanceof Element)) {
    return;
  }
  const element = node;
  const name = element.tagName.toLowerCase();
  if (HIDDEN.has(name)) {
    return;
  }
  if (name === 'img') {
    const src = element.getAttribute('src') ?? '';
    const alt = element.getAttribute('alt') ?? '';
    atoms.push(`i${FIELD}${src}${FIELD}${alt}`);
    return;
  }
  if (name === 'a') {
    if (linked.length > 0) {
      atoms.push('z');
    }
    const href = element.getAttribute('href') ?? '';
    atoms.push(`a${FIELD}${href}`);
    linked.push(true);
    for (const child of element.childNodes) {
      atomsOf(child, atoms, linked);
    }
    atoms.push('z');
    linked.pop();
    return;
  }
  for (const child of element.childNodes) {
    atomsOf(child, atoms, linked);
  }
};

/**
 * The canonical content form of `html`: the ordered visible atoms, joined.
 *
 * It matches `crates/omeka-site/src/blocks/canonical.rs`, through `DOMParser`.
 * The generated registry carries a worked sample both sides test against.
 */
export const canonicalContent = (html: string): string => {
  const document = new DOMParser().parseFromString(html, 'text/html');
  const atoms: string[] = [];
  atomsOf(document.body, atoms, []);
  return atoms.join(ATOM);
};

/** FNV-1a 64 over the canonical content, as 16 lowercase hex digits. */
export const plainHash = (text: string): string => fnv1a(canonicalContent(text));

/** The carrier version whose `plain_hash` is the raw-byte hash. */
export const LEGACY_CARRIER_VERSION = 1;

/** Whether a carrier's stored hash vouches for `html`, as `romeka.rs` decides. */
export const storedMatches = (carrier: RomkaData, html: string): boolean => {
  if (carrier.plainHash === null) {
    return false;
  }
  if (carrier.plainHash === plainHash(html)) {
    return true;
  }
  return carrier.v <= LEGACY_CARRIER_VERSION && carrier.plainHash === legacyPlainHash(html);
};

/**
 * The carrier states the editor distinguishes.
 *
 * `unhashed` is a block that has not been through a Romeka save yet; `edited`
 * is one whose stored html was changed outside Romeka, which the server renders
 * as stored until the user asks for the Romeka version.
 */
export type RomkaState = 'none' | 'unhashed' | 'ok' | 'edited';

/** The state of a block's carrier, `none` when it carries no registered block. */
export const romkaState = (
  registry: BlockRegistry,
  block: { readonly data: JsonValue; readonly layoutData: LayoutData | null },
): RomkaState => {
  if (romkaFor(registry, block) === undefined) {
    return 'none';
  }
  const carrier = carrierOf(block);
  if (carrier === null || carrier.plainHash === null) {
    return 'unhashed';
  }
  const html =
    isJsonObject(block.data) && typeof block.data.html === 'string' ? block.data.html : '';
  return storedMatches(carrier, html) ? 'ok' : 'edited';
};
