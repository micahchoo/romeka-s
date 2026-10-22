/**
 * W3C Web Annotation JSON and the admin's view of a note.
 *
 * This is the only file that knows the W3C key names. Everything outside it
 * holds `Draft`, `Note` and `Container` (contract section 5), so the panel,
 * the region editor and the time editor never spell a selector.
 *
 * A stored annotation's page document carries no revision, so `fromW3C`
 * leaves `etag` empty; `client.ts` fills it from the answer that carried the
 * note, and resolves an empty one with a `GET` before a conditional write.
 */
import {
  ParseError,
  asRecord,
  asString,
  field,
  isRecord,
  isUnknownArray,
  parseListOf,
} from '../../../api/parse';
import type { Path } from '../../../api/types';

const ANNO_CONTEXT = 'http://www.w3.org/ns/anno.jsonld';
const MEDIA_FRAGS = 'http://www.w3.org/TR/media-frags/';
const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

/** What a selector on a media may point at (contract 4.1). */
export type Surface =
  | { readonly kind: 'image'; readonly width: number; readonly height: number; readonly service: string }
  | {
      readonly kind: 'time';
      readonly durationMs: number | null;
      /** A video's frame in the pixels its boxes are drawn in; null for a sound. */
      readonly frame: { readonly width: number; readonly height: number } | null;
    }
  | { readonly kind: 'whole' };

/** The canonical selector forms the server writes (contract 6.3). */
export type Selector =
  | { readonly kind: 'whole' }
  | { readonly kind: 'rect'; readonly x: number; readonly y: number; readonly w: number; readonly h: number }
  | { readonly kind: 'polygon'; readonly points: readonly (readonly [number, number])[] }
  | { readonly kind: 'time'; readonly startMs: number; readonly endMs: number }
  /** A box on a video's frames while the player is inside its range. */
  | {
      readonly kind: 'box';
      readonly x: number;
      readonly y: number;
      readonly w: number;
      readonly h: number;
      readonly startMs: number;
      readonly endMs: number;
    };

/** The range a selector covers on a recording: a time note's or a box's. */
export const spanOf = (selector: Selector): { readonly startMs: number; readonly endMs: number } | null =>
  selector.kind === 'time' || selector.kind === 'box'
    ? { startMs: selector.startMs, endMs: selector.endMs }
    : null;

/** The selector with its range moved, or as it was when it has none. */
export const withSpan = (selector: Selector, startMs: number, endMs: number): Selector =>
  selector.kind === 'time' || selector.kind === 'box' ? { ...selector, startMs, endMs } : selector;

export type Motivation = 'commenting' | 'tagging' | 'describing' | 'supplementing';

/** What a caller may set on a note. */
export interface Draft {
  readonly motivation: Motivation;
  readonly selector: Selector;
  readonly text: string;
  readonly language: string | null;
}

/**
 * One stored annotation in the admin's terms. `raw` is the server's own
 * document: a `PUT` sends it back with the edited fields applied, so another
 * client's extension keys survive (contract 5).
 */
export interface Note extends Omit<Draft, 'motivation'> {
  readonly iri: string;
  /** The strong ETag `"r<n>"`, or empty when the note came from a page. */
  readonly etag: string;
  /** A motivation the admin knows, or another one a WAP client stored. */
  readonly motivation: string;
  readonly fromTrack: boolean;
  /** The server's `modified`, as it writes it (ISO 8601), or null. */
  readonly modified: string | null;
  readonly raw: unknown;
}

export interface Container {
  readonly surface: Surface;
  /** The media canvas IRI: `Romeka-Canvas`, else the target of the first note. */
  readonly canvas: string;
  readonly writable: boolean;
  readonly total: number;
  readonly notes: readonly Note[];
}

/** An optional string member, kept as null when absent or null. */
const optionalString = (object: Record<string, unknown>, key: string): string | null => {
  const value = object[key];
  return typeof value === 'string' ? value : null;
};

/**
 * Read `Romeka-Surface` (contract 6.2), an RFC 8941 item:
 * `image; width=4000; height=3000; service="https://…"`, `time; duration=62.5`
 * or `whole`.
 */
export const parseSurface = (header: string | null): Surface => {
  const item = parseItem(header);
  switch (item.name) {
    case 'whole':
      return { kind: 'whole' };
    case 'time': {
      const duration = item.params.get('duration');
      const width = item.params.get('width');
      const height = item.params.get('height');
      return {
        kind: 'time',
        durationMs: duration === undefined ? null : secondsToMs(duration, ['duration']),
        frame:
          width === undefined || height === undefined
            ? null
            : { width: positiveInteger(width, ['width']), height: positiveInteger(height, ['height']) },
      };
    }
    case 'image': {
      const service = item.params.get('service');
      if (service === undefined || service === '') {
        throw new ParseError(['service'], 'the Image API service IRI');
      }
      return {
        kind: 'image',
        width: positiveInteger(item.params.get('width'), ['width']),
        height: positiveInteger(item.params.get('height'), ['height']),
        service,
      };
    }
    default:
      throw new ParseError([], `image, time or whole, not ${item.name}`);
  }
};

interface HeaderItem {
  readonly name: string;
  readonly params: ReadonlyMap<string, string>;
}

const parseItem = (header: string | null): HeaderItem => {
  if (header === null || header.trim() === '') {
    throw new ParseError([], 'a Romeka-Surface header');
  }
  const segments = splitOutsideQuotes(header);
  const name = segments[0]?.trim() ?? '';
  if (name === '') {
    throw new ParseError([], 'a surface name');
  }
  const params = new Map<string, string>();
  for (const segment of segments.slice(1)) {
    const trimmed = segment.trim();
    if (trimmed === '') {
      continue;
    }
    const equals = trimmed.indexOf('=');
    if (equals < 0) {
      params.set(trimmed, '');
      continue;
    }
    params.set(trimmed.slice(0, equals).trim(), unquote(trimmed.slice(equals + 1).trim()));
  }
  return { name, params };
};

/** Split an RFC 8941 item on `;`, leaving a quoted string's own `;` alone. */
const splitOutsideQuotes = (header: string): readonly string[] => {
  const parts: string[] = [];
  let current = '';
  let quoted = false;
  for (const character of header) {
    if (character === '"') {
      quoted = !quoted;
      current += character;
    } else if (character === ';' && !quoted) {
      parts.push(current);
      current = '';
    } else {
      current += character;
    }
  }
  parts.push(current);
  return parts;
};

const unquote = (value: string): string => {
  if (value.length < 2 || !value.startsWith('"') || !value.endsWith('"')) {
    return value;
  }
  return value.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, '\\');
};

const positiveInteger = (text: string | undefined, path: Path): number => {
  const value = text === undefined ? Number.NaN : Number.parseInt(text, 10);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new ParseError(path, 'a positive integer');
  }
  return value;
};

const secondsToMs = (text: string, path: Path): number => {
  const seconds = Number.parseFloat(text);
  if (!Number.isFinite(seconds) || seconds < 0) {
    throw new ParseError(path, 'seconds');
  }
  return Math.round(seconds * 1000);
};

/** The canvas an annotation document targets, or null when it names none. */
export const targetSourceOf = (raw: unknown): string | null => {
  if (!isRecord(raw)) {
    return null;
  }
  const target = raw['target'];
  if (typeof target === 'string') {
    return target;
  }
  if (!isRecord(target)) {
    return null;
  }
  const source = target['source'];
  if (typeof source === 'string') {
    return source;
  }
  return isRecord(source) ? optionalString(source, 'id') : null;
};

/** A server document as the admin's note. `trackIds` mark caption-import rows. */
export const fromW3C = (json: unknown, trackIds: ReadonlySet<string>): Note => {
  const document = asRecord(json, []);
  const iri = asString(field(document, 'id', []), ['id']);
  return {
    iri,
    etag: '',
    motivation: motivationOf(document['motivation']),
    selector: selectorOf(field(document, 'target', []), ['target']),
    ...textOf(document['body']),
    modified: optionalString(document, 'modified'),
    fromTrack: trackIds.has(noteIdOf(iri)),
    raw: json,
  };
};

/**
 * The last segment of a note's IRI, which is its row id: what
 * `?note=` in the admin carries (`notes` link module).
 */
export const noteIdOf = (iri: string): string => {
  const slash = iri.lastIndexOf('/');
  return slash < 0 ? iri : iri.slice(slash + 1);
};

const motivationOf = (value: unknown): string => {
  const motivation = isUnknownArray(value) ? value[0] : value;
  return asString(motivation, ['motivation']);
};

const textOf = (body: unknown): { readonly text: string; readonly language: string | null } => {
  const bodies = isUnknownArray(body) ? body : body === undefined ? [] : [body];
  for (const entry of bodies) {
    if (!isRecord(entry) || entry['type'] !== 'TextualBody') {
      continue;
    }
    const value = entry['value'];
    return {
      text: value === undefined ? '' : asString(value, ['body', 'value']),
      language: optionalString(entry, 'language'),
    };
  }
  return { text: '', language: null };
};

const selectorOf = (target: unknown, path: Path): Selector => {
  if (typeof target === 'string') {
    return { kind: 'whole' };
  }
  const resource = asRecord(target, path);
  const source = resource['source'];
  if (source === undefined) {
    throw new ParseError([...path, 'source'], 'a canvas IRI');
  }
  const selector = resource['selector'];
  if (selector === undefined || selector === null) {
    return { kind: 'whole' };
  }
  const record = asRecord(selector, [...path, 'selector']);
  const type = asString(field(record, 'type', [...path, 'selector']), [...path, 'selector', 'type']);
  if (type === 'SvgSelector') {
    return { kind: 'polygon', points: pointsOf(record['value'], [...path, 'selector', 'value']) };
  }
  if (type !== 'FragmentSelector') {
    throw new ParseError([...path, 'selector', 'type'], 'a FragmentSelector or SvgSelector');
  }
  const value = asString(field(record, 'value', [...path, 'selector']), [...path, 'selector', 'value']);
  return fragmentOf(value, [...path, 'selector', 'value']);
};

/**
 * A media fragment: `xywh=`, `t=`, or both joined by `&` in either order (a box
 * on a video's frames over a range).
 */
const fragmentOf = (value: string, path: Path): Selector => {
  let rect: Extract<Selector, { kind: 'rect' }> | null = null;
  let time: Extract<Selector, { kind: 'time' }> | null = null;
  for (const part of value.split('&')) {
    const box = /^xywh=(?:pixel:)?(\d+(?:\.\d+)?),(\d+(?:\.\d+)?),(\d+(?:\.\d+)?),(\d+(?:\.\d+)?)$/.exec(part);
    const range = /^t=(\d+(?:\.\d+)?),(\d+(?:\.\d+)?)$/.exec(part);
    if (box !== null && rect === null) {
      const [x, y, w, h] = box.slice(1).map((number) => Math.round(Number(number)));
      if (x === undefined || y === undefined || w === undefined || h === undefined) {
        throw new ParseError(path, 'a pixel box');
      }
      rect = { kind: 'rect', x, y, w, h };
    } else if (range !== null && time === null) {
      const start = Number(range[1]);
      const end = Number(range[2]);
      time = { kind: 'time', startMs: Math.round(start * 1000), endMs: Math.round(end * 1000) };
    } else {
      throw new ParseError(path, 'a pixel box, a time range, or both');
    }
  }
  if (rect !== null && time !== null) {
    return { ...rect, kind: 'box', startMs: time.startMs, endMs: time.endMs };
  }
  const only = rect ?? time;
  if (only === null) {
    throw new ParseError(path, 'a pixel box or a time range');
  }
  return only;
};

const POINT_PATTERN = /(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)/g;

const pointsOf = (value: unknown, path: Path): readonly (readonly [number, number])[] => {
  const svg = asString(value, path);
  const points: (readonly [number, number])[] = [];
  for (const match of svg.matchAll(POINT_PATTERN)) {
    const x = Number(match[1]);
    const y = Number(match[2]);
    if (Number.isFinite(x) && Number.isFinite(y)) {
      points.push([Math.round(x), Math.round(y)]);
    }
  }
  if (points.length === 0) {
    throw new ParseError(path, 'an SVG polygon');
  }
  return points;
};

/**
 * The W3C document for a draft. `base` is the server's last document for this
 * note: its members are kept, and only the draft's fields are replaced, so
 * extension keys a client cannot render survive the round trip.
 */
export const toW3C = (draft: Draft, canvas: string, base?: unknown): Record<string, unknown> => {
  const document: Record<string, unknown> = isRecord(base) ? { ...base } : {};
  document['@context'] = ANNO_CONTEXT;
  document['type'] = 'Annotation';
  document['motivation'] = draft.motivation;
  document['target'] = targetOf(draft, canvas, document['target']);
  const body = bodyOf(document['body'], draft);
  if (body === undefined) {
    delete document['body'];
  } else {
    document['body'] = body;
  }
  return document;
};

const targetOf = (draft: Draft, canvas: string, base: unknown): unknown => {
  if (draft.selector.kind === 'whole') {
    return canvas;
  }
  const previous = isRecord(base) ? base : {};
  const source = previous['source'] ?? { id: canvas, type: 'Canvas' };
  return { ...previous, type: 'SpecificResource', source, selector: selectorJson(draft.selector) };
};

const selectorJson = (selector: Selector): Record<string, unknown> => {
  switch (selector.kind) {
    case 'rect':
      return {
        type: 'FragmentSelector',
        conformsTo: MEDIA_FRAGS,
        value: `xywh=pixel:${String(Math.round(selector.x))},${String(Math.round(selector.y))},${String(Math.round(selector.w))},${String(Math.round(selector.h))}`,
      };
    case 'polygon': {
      const points = selector.points
        .map(([x, y]) => `${String(Math.round(x))},${String(Math.round(y))}`)
        .join(' ');
      return {
        type: 'SvgSelector',
        value: `<svg xmlns="${SVG_NAMESPACE}"><polygon points="${points}"/></svg>`,
      };
    }
    case 'time':
      return {
        type: 'FragmentSelector',
        conformsTo: MEDIA_FRAGS,
        value: `t=${seconds(selector.startMs)},${seconds(selector.endMs)}`,
      };
    case 'box':
      return {
        type: 'FragmentSelector',
        conformsTo: MEDIA_FRAGS,
        value: `xywh=pixel:${String(Math.round(selector.x))},${String(Math.round(selector.y))},${String(Math.round(selector.w))},${String(Math.round(selector.h))}&t=${seconds(selector.startMs)},${seconds(selector.endMs)}`,
      };
    case 'whole':
      return {};
  }
};

/** Seconds with at most three decimals, trailing zeros dropped (`70.5`). */
const seconds = (milliseconds: number): string =>
  (milliseconds / 1000).toFixed(3).replace(/\.?0+$/, '');

const bodyOf = (base: unknown, draft: Draft): unknown => {
  const bodies = isUnknownArray(base) ? [...base] : base === undefined ? [] : [base];
  const index = bodies.findIndex((entry) => isRecord(entry) && entry['type'] === 'TextualBody');
  if (index < 0) {
    if (draft.text === '') {
      return bodies.length === 1 ? bodies[0] : bodies.length === 0 ? undefined : bodies;
    }
    bodies.push(textBody(draft));
  } else {
    bodies[index] = textBody(draft);
  }
  return bodies.length === 1 ? bodies[0] : bodies;
};

const textBody = (draft: Draft): Record<string, unknown> => {
  const body: Record<string, unknown> = { type: 'TextualBody', value: draft.text, format: 'text/plain' };
  if (draft.language !== null) {
    body['language'] = draft.language;
  }
  return body;
};

/** The notes of one page document, in document order. */
export const notesOf = (page: unknown, trackIds: ReadonlySet<string>): readonly Note[] =>
  parseListOf((entry) => fromW3C(entry, trackIds))(
    field(asRecord(page, []), 'items', []),
    ['items'],
  );

/** `Romeka-Track-Ids` holds the ids of the track rows on the page it answers. */
export const trackIdsOf = (header: string | null): ReadonlySet<string> =>
  new Set(header === null ? [] : header.split(/[\s,]+/).filter((id) => id !== ''));
