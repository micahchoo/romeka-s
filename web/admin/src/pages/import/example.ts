/**
 * "Mark an example": one file path, cut into folders and name parts, each part
 * marked with what it means, compiled to the plan's `pattern`.
 *
 * The pattern stays inside what both JavaScript and the Rust `regex` crate
 * accept, because the server compiles it there: named groups `(?<name>…)`,
 * character classes, fixed counts and optional non-capturing groups. Digits are
 * `[0-9]`, not `\d`, which the Rust crate reads as any Unicode digit.
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

/** Text of the same kind: the same number of digits, capitals, small letters, or any of them. */
const kindOf = (text: string): string => {
  if (/^[0-9]+$/.test(text)) {
    return `[0-9]{${String(text.length)}}`;
  }
  if (/^[A-Z]+$/.test(text)) {
    return '[A-Z]+';
  }
  if (/^[a-z]+$/.test(text)) {
    return '[a-z]+';
  }
  return '[A-Za-z0-9]+';
};

const escapeDelimiter = (text: string): string => (text === '.' ? '\\.' : text);

interface Piece {
  /** `null` for a delimiter, `undefined` for an unmarked part. */
  readonly label: Label | null | undefined;
  pattern: string;
  text: string;
}

/**
 * Consecutive name parts with one label, with or without a delimiter between
 * them, are one group: `BHC`, `006`, `AF`, `001` marked identifier are the
 * identifier `BHC_006_AF_001`.
 */
const namePieces = (parts: ExampleParts, labels: ImportExample['labels']): readonly Piece[] => {
  const pieces: Piece[] = [];
  for (const part of parts.name) {
    if (part.delimiter) {
      pieces.push({ label: null, pattern: escapeDelimiter(part.text), text: part.text });
      continue;
    }
    const label = labels[part.key];
    const last = pieces.at(-1);
    const beforeLast = pieces.at(-2);
    if (label !== undefined && last?.label === label) {
      last.pattern += kindOf(part.text);
      last.text += part.text;
    } else if (label !== undefined && last?.label === null && beforeLast?.label === label) {
      pieces.pop();
      beforeLast.pattern += last.pattern + kindOf(part.text);
      beforeLast.text += last.text + part.text;
    } else {
      pieces.push({ label, pattern: kindOf(part.text), text: part.text });
    }
  }
  return pieces;
};

/** A group name not yet taken: `identifier`, then `identifier2`. */
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

/**
 * Folders up to the last marked one are one segment each; any depth of folders
 * may follow, so `Photographs/1950s/PH_101.tif` fits an example one folder deep.
 */
const compileFolders = (
  parts: ExampleParts,
  labels: ImportExample['labels'],
  used: Set<string>,
): CompiledExample => {
  if (parts.folders.length === 0) {
    return { pattern: '', shape: '' };
  }
  let last = -1;
  parts.folders.forEach((_folder, index) => {
    if (labels[folderKey(index)] !== undefined) {
      last = index;
    }
  });
  let pattern = '';
  let shape = '';
  for (let index = 0; index <= last; index += 1) {
    const label = labels[folderKey(index)];
    if (label === undefined) {
      pattern += '[^/]+/';
      shape += `[${parts.folders[index] ?? ''}]/`;
    } else {
      const name = claim(used, label);
      pattern += `(?<${name}>[^/]+)/`;
      shape += `{${name}}/`;
    }
  }
  return { pattern: `${pattern}(?:[^/]+/)*`, shape: `${shape}…/` };
};

export const compileExample = (example: ImportExample): CompiledExample => {
  const parts = splitExample(example.path);
  const used = new Set<string>();
  const folders = compileFolders(parts, example.labels, used);
  const pieces = namePieces(parts, example.labels);
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
    if (example.optional.includes(piece.label) && previous?.label === null) {
      // The delimiter before an optional part is optional with it.
      pattern = `${pattern.slice(0, pattern.length - previous.pattern.length)}(?:${previous.pattern}${group})?`;
      shape = `${shape.slice(0, shape.length - previous.text.length)}[${previous.text}{${name}}]`;
    } else {
      pattern += group;
      shape += `{${name}}`;
    }
  });
  const ext = parts.ext === '' ? { pattern: '', shape: '' } : { pattern: '\\.(?<ext>[A-Za-z0-9]+)', shape: '.{ext}' };
  return {
    pattern: `^${folders.pattern}${pattern}${ext.pattern}$`,
    shape: `${folders.shape}${shape}${ext.shape}`,
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
