/**
 * "Mark an example": one file path, cut into folders and name parts, each part
 * marked with what it means, compiled to the plan's `pattern`.
 *
 * The pattern stays inside what both JavaScript and the Rust `regex` crate
 * accept, because the server compiles it there: named groups `(?<name>…)`,
 * character classes and optional non-capturing groups. Letters are `\p{L}`
 * plus `\p{M}`, because a Devanagari vowel sign is a mark and not a letter;
 * digits are `\p{Nd}`, because the archive holds Tamil digits too. JavaScript
 * needs the `u` flag to read either property, so a caller builds its `RegExp`
 * with `u`.
 */
import type { ImportExample, Label } from '../../api/imports';

/** One name part or the delimiter between two. */
export interface NamePart {
  /** `n<index>`, the key the example's labels use. */
  readonly key: string;
  readonly text: string;
  readonly delimiter: boolean;
}

export interface ExampleParts {
  /** The folders above the file, top first; folder `i` has the key `d<i>`. */
  readonly folders: readonly string[];
  readonly name: readonly NamePart[];
  readonly ext: string;
}

export interface CompiledExample {
  readonly pattern: string;
  /** The readable form: `{set}/…/{identifier}[_{part}].{ext}`. */
  readonly shape: string;
}

const DELIMITER = /^[_\-. ]$/;
const DIGITS = /^\p{Nd}+$/u;
const LETTERS = /^[\p{L}\p{M}]+$/u;

export const folderKey = (index: number): string => `d${String(index)}`;

export const splitExample = (path: string): ExampleParts => {
  const segments = path.split('/').filter((segment) => segment !== '');
  const file = segments.pop() ?? '';
  const dot = file.lastIndexOf('.');
  const base = dot > 0 ? file.slice(0, dot) : file;
  return {
    folders: segments,
    ext: dot > 0 ? file.slice(dot + 1) : '',
    name: base
      .split(/([_\-. ])/)
      .filter((text) => text !== '')
      .map((text, index) => ({ key: `n${String(index)}`, text, delimiter: DELIMITER.test(text) })),
  };
};

/**
 * The labels a part may take. Every folder may be the identifier or the page,
 * so a folder per object works; only the top folder names an item set, and
 * only a name part tells files of one item apart.
 */
export const labelsFor = (key: string): readonly Label[] => {
  if (key === folderKey(0)) {
    return ['set', 'identifier', 'page'];
  }
  return key.startsWith('d') ? ['identifier', 'page'] : ['identifier', 'part', 'page'];
};

/**
 * Text of the same kind: one or more digits in any script, one or more letters
 * and their combining marks in any case, or a run of both.
 */
const kindOf = (text: string): string => {
  if (DIGITS.test(text)) {
    return '\\p{Nd}+';
  }
  if (LETTERS.test(text)) {
    return '[\\p{L}\\p{M}]+';
  }
  return '[\\p{L}\\p{M}\\p{N}]+';
};

const escapeDelimiter = (text: string): string => (text === '.' ? '\\.' : text);

interface Piece {
  /** `null` for a literal or the folder gap, `undefined` for an unmarked part. */
  readonly label: Label | null | undefined;
  readonly pattern: string;
  readonly text: string;
  /** The gap that lets any depth of folders follow the last marked one. */
  readonly gap?: boolean;
}

/**
 * Folders up to the last marked one are one segment each; any depth of folders
 * may follow, so `Photographs/1950s/PH_101.tif` fits an example one folder deep.
 */
const folderPieces = (parts: ExampleParts, labels: ImportExample['labels']): readonly Piece[] => {
  if (parts.folders.length === 0) {
    return [];
  }
  let last = -1;
  parts.folders.forEach((_folder, index) => {
    if (labels[folderKey(index)] !== undefined) {
      last = index;
    }
  });
  const pieces: Piece[] = [];
  for (let index = 0; index <= last; index += 1) {
    pieces.push({ label: labels[folderKey(index)], pattern: '[^/]+', text: parts.folders[index] ?? '' });
    pieces.push({ label: null, pattern: '/', text: '/' });
  }
  pieces.push({ label: null, pattern: '(?:[^/]+/)*', text: '…/', gap: true });
  return pieces;
};

const namePieces = (parts: ExampleParts, labels: ImportExample['labels']): readonly Piece[] =>
  parts.name.map((part) =>
    part.delimiter
      ? { label: null, pattern: escapeDelimiter(part.text), text: part.text }
      : { label: labels[part.key], pattern: kindOf(part.text), text: part.text },
  );

/**
 * Every identifier mark is one group, never `identifier2`: the server reads
 * only `identifier`, so `Box 4` and `Folder 12` marked identifier must read as
 * `Box 4/Folder 12`, and two such folders stay two items. Everything between
 * the first and last mark joins the group, separators included; a contradictory
 * label between them is folded in rather than splitting the identifier.
 */
const mergeIdentifiers = (pieces: readonly Piece[]): readonly Piece[] => {
  const merged: Piece[] = [];
  let index = 0;
  while (index < pieces.length) {
    const piece = pieces[index];
    if (piece === undefined) {
      break;
    }
    if (piece.label !== 'identifier') {
      merged.push(piece);
      index += 1;
      continue;
    }
    let end = index;
    for (let scan = pieces.length - 1; scan > index; scan -= 1) {
      if (pieces[scan]?.label === 'identifier') {
        end = scan;
        break;
      }
    }
    let pattern = '';
    let text = '';
    for (let scan = index; scan <= end; scan += 1) {
      const part = pieces[scan];
      if (part === undefined) {
        continue;
      }
      pattern += part.pattern;
      text += part.text;
    }
    merged.push({ label: 'identifier', pattern, text });
    index = end + 1;
  }
  return merged;
};

/** A group name not yet taken: `identifier`, `part`, and so on. */
const claim = (used: Set<string>, label: Label): string => {
  let name: string = label;
  let n = 2;
  while (used.has(name)) {
    name = `${label}${String(n)}`;
    n += 1;
  }
  used.add(name);
  return name;
};

export const compileExample = (example: ImportExample): CompiledExample => {
  const parts = splitExample(example.path);
  const used = new Set<string>();
  const pieces = mergeIdentifiers([...folderPieces(parts, example.labels), ...namePieces(parts, example.labels)]);
  let pattern = '';
  let shape = '';
  pieces.forEach((piece, index) => {
    if (piece.label === null) {
      pattern += piece.pattern;
      shape += piece.text;
      return;
    }
    if (piece.label === undefined) {
      pattern += piece.pattern;
      shape += `[${piece.text}]`;
      return;
    }
    const name = claim(used, piece.label);
    const group = `(?<${name}>${piece.pattern})`;
    const previous = pieces[index - 1];
    if (example.optional.includes(piece.label) && previous?.label === null && previous.gap !== true) {
      // The delimiter before an optional part is optional with it.
      pattern = `${pattern.slice(0, pattern.length - previous.pattern.length)}(?:${previous.pattern}${group})?`;
      shape = `${shape.slice(0, shape.length - previous.text.length)}[${previous.text}{${name}}]`;
    } else {
      pattern += group;
      shape += `{${name}}`;
    }
  });
  const ext = parts.ext === '' ? { pattern: '', shape: '' } : { pattern: '\\.(?<ext>[\\p{L}\\p{M}\\p{N}]+)', shape: '.{ext}' };
  return {
    pattern: `^${pattern}${ext.pattern}$`,
    shape: `${shape}${ext.shape}`,
  };
};

/**
 * The marks a new example starts with: the whole name is the identifier, so
 * each file is one item, and the top folder names its item set.
 */
export const defaultExample = (path: string): ImportExample => {
  const parts = splitExample(path);
  const labels: Record<string, Label> = {};
  if (parts.folders.length > 0) {
    labels[folderKey(0)] = 'set';
  }
  for (const part of parts.name) {
    if (!part.delimiter) {
      labels[part.key] = 'identifier';
    }
  }
  return { path, labels, optional: [] };
};

/**
 * The labels "Make it optional" could apply to: marked on a name part after a
 * delimiter, not the identifier, and not optional yet.
 */
export const optionalCandidates = (example: ImportExample): readonly Label[] => {
  const parts = splitExample(example.path);
  const found = new Set<Label>();
  parts.name.forEach((part, index) => {
    const label = example.labels[part.key];
    if (
      label !== undefined &&
      label !== 'identifier' &&
      !example.optional.includes(label) &&
      parts.name[index - 1]?.delimiter === true
    ) {
      found.add(label);
    }
  });
  return [...found];
};

/** The optional labels still marked somewhere, which "Make it required" lists. */
export const optionalInUse = (example: ImportExample): readonly Label[] => {
  const marked = new Set(Object.values(example.labels));
  return example.optional.filter((label) => marked.has(label));
};

/** The example with `key` marked `label`, or unmarked when `label` is `null`. */
export const withLabel = (example: ImportExample, key: string, label: Label | null): ImportExample => {
  const others = Object.entries(example.labels).filter(([entry]) => entry !== key);
  return { ...example, labels: Object.fromEntries(label === null ? others : [...others, [key, label]]) };
};

export const withOptional = (example: ImportExample, label: Label, optional: boolean): ImportExample => ({
  ...example,
  optional: optional
    ? [...example.optional.filter((entry) => entry !== label), label]
    : example.optional.filter((entry) => entry !== label),
});
