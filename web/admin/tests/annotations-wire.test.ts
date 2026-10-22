/**
 * The admin's wire mapping (contract section 5 and 6.3): a stored annotation
 * becomes a `Note`, a `Draft` becomes the document a write sends, and
 * `Romeka-Surface` becomes the surface the editor may draw on. The fixtures
 * are the wire documents of contract 6.2 and 6.3, copied byte for byte.
 */
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { ParseError } from '../src/api/parse';
import {
  fromW3C,
  notesOf,
  parseSurface,
  targetSourceOf,
  toW3C,
  trackIdsOf,
} from '../src/pages/media/annotations/wire';
import type { Draft, Note, Selector } from '../src/pages/media/annotations/wire';
import { loadFixture } from './loadFixture';

const CANVAS = 'https://archive.example/iiif/3/item/812/canvas/4127';

const firstNote = (raw: unknown, trackIds: ReadonlySet<string> = new Set()): Note => {
  const note = notesOf(raw, trackIds)[0];
  if (note === undefined) {
    throw new Error('the fixture holds one annotation');
  }
  return note;
};

const rect: Selector = { kind: 'rect', x: 1200, y: 340, w: 410, h: 388 };

describe('fromW3C', () => {
  it('reads the contract 6.3 annotation as a rect note', () => {
    const note = firstNote(loadFixture('annotation-page.json'));
    expect(note.iri).toBe('https://archive.example/annotations/90311');
    expect(note.motivation).toBe('commenting');
    expect(note.selector).toEqual(rect);
    expect(note.text).toBe('Watermark: crowned shield');
    expect(note.language).toBe('en');
    expect(note.modified).toBe('2026-10-20T09:12:44Z');
    expect(note.fromTrack).toBe(false);
    expect(note.etag).toBe('');
    expect(targetSourceOf(note.raw)).toBe(CANVAS);
  });

  it('marks the notes the page lists in Romeka-Track-Ids', () => {
    const page = loadFixture('annotation-page.json');
    expect(firstNote(page, trackIdsOf('90311 92001')).fromTrack).toBe(true);
    expect(firstNote(page, trackIdsOf(null)).fromTrack).toBe(false);
  });

  it('reads a whole-file note from a bare IRI target', () => {
    const note = fromW3C(
      {
        id: 'https://archive.example/annotations/90400',
        type: 'Annotation',
        motivation: 'describing',
        body: { type: 'TextualBody', value: 'Foxed throughout.', format: 'text/plain' },
        target: CANVAS,
      },
      new Set(),
    );
    expect(note.selector).toEqual({ kind: 'whole' });
    expect(note.language).toBeNull();
    expect(note.modified).toBeNull();
  });

  it('reads a time range at millisecond precision', () => {
    const note = fromW3C(
      {
        id: 'https://archive.example/annotations/90401',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: 'The speaker names the place', language: 'en' },
        target: {
          source: { id: CANVAS },
          selector: { type: 'FragmentSelector', value: 't=62,70.5' },
        },
      },
      new Set(),
    );
    expect(note.selector).toEqual({ kind: 'time', startMs: 62000, endMs: 70500 });
  });

  it('reads a box on a video frame over a time range, in either order', () => {
    const read = (value: string): Selector =>
      fromW3C(
        {
          id: 'https://archive.example/annotations/90406',
          motivation: 'commenting',
          body: { type: 'TextualBody', value: 'The speaker\'s hands' },
          target: { source: { id: CANVAS }, selector: { type: 'FragmentSelector', value } },
        },
        new Set(),
      ).selector;
    const box = { kind: 'box', x: 10, y: 20, w: 300, h: 150, startMs: 1500, endMs: 4000 };
    expect(read('xywh=pixel:10,20,300,150&t=1.5,4')).toEqual(box);
    expect(read('t=1.5,4&xywh=10,20,300,150')).toEqual(box);
  });

  it('reads a polygon from the canonical SvgSelector', () => {
    const note = fromW3C(
      {
        id: 'https://archive.example/annotations/90402',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: 'Watermark' },
        target: {
          source: { id: CANVAS },
          selector: {
            type: 'SvgSelector',
            value: '<svg xmlns="http://www.w3.org/2000/svg"><polygon points="1200,340 1610,340 1610,728"/></svg>',
          },
        },
      },
      new Set(),
    );
    expect(note.selector).toEqual({
      kind: 'polygon',
      points: [
        [1200, 340],
        [1610, 340],
        [1610, 728],
      ],
    });
  });

  it('reads only the points out of an SvgSelector that carries markup', () => {
    const note = fromW3C(
      {
        id: 'https://archive.example/annotations/90403',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: 'Watermark' },
        target: {
          source: { id: CANVAS },
          selector: {
            type: 'SvgSelector',
            value: '<svg><script>alert(1)</script><polygon points="10,20 30,40 50,60"/></svg>',
          },
        },
      },
      new Set(),
    );
    expect(note.selector).toEqual({
      kind: 'polygon',
      points: [
        [10, 20],
        [30, 40],
        [50, 60],
      ],
    });
  });

  it('refuses a dashed SvgSelector', () => {
    expect(() =>
      fromW3C(
        {
          id: 'https://archive.example/annotations/90404',
          motivation: 'commenting',
          target: { source: { id: CANVAS }, selector: { type: 'SvgSelector', value: '<svg/>' } },
        },
        new Set(),
      ),
    ).toThrow(ParseError);
  });

  it('refuses a percent box, which needs a canvas size it does not have', () => {
    expect(() =>
      fromW3C(
        {
          id: 'https://archive.example/annotations/90405',
          motivation: 'commenting',
          target: { source: { id: CANVAS }, selector: { type: 'FragmentSelector', value: 'xywh=percent:30,10,10,10' } },
        },
        new Set(),
      ),
    ).toThrow(ParseError);
  });
});

describe('toW3C', () => {
  const draft = (selector: Selector, text = 'Watermark: crowned shield'): Draft => ({
    motivation: 'commenting',
    selector,
    text,
    language: 'en',
  });

  const roundTrip = (value: Draft): Note =>
    fromW3C(toW3C(value, CANVAS, { id: 'https://archive.example/annotations/90311' }), new Set());

  it.each([
    ['a rect', rect],
    [
      'a polygon',
      {
        kind: 'polygon',
        points: [
          [1200, 340],
          [1610, 340],
          [1610, 728],
        ],
      },
    ],
    ['a time range', { kind: 'time', startMs: 62000, endMs: 70500 }],
    ['a box over a time range', { kind: 'box', x: 10, y: 20, w: 300, h: 150, startMs: 1500, endMs: 4000 }],
    ['the whole file', { kind: 'whole' }],
  ] as const)('round-trips %s', (_name, selector) => {
    const value = draft(selector);
    const note = roundTrip(value);
    expect({
      motivation: note.motivation,
      selector: note.selector,
      text: note.text,
      language: note.language,
    }).toEqual(value);
  });

  it('writes the canonical selector forms', () => {
    expect(toW3C(draft(rect), CANVAS)['target']).toMatchObject({
      type: 'SpecificResource',
      source: { id: CANVAS, type: 'Canvas' },
      selector: {
        type: 'FragmentSelector',
        conformsTo: 'http://www.w3.org/TR/media-frags/',
        value: 'xywh=pixel:1200,340,410,388',
      },
    });
    expect(toW3C(draft({ kind: 'time', startMs: 62000, endMs: 62500 }), CANVAS)['target']).toMatchObject({
      selector: { value: 't=62,62.5' },
    });
    expect(
      toW3C(draft({ kind: 'box', x: 10, y: 20, w: 300, h: 150, startMs: 1500, endMs: 4000 }), CANVAS)['target'],
    ).toMatchObject({
      selector: { type: 'FragmentSelector', value: 'xywh=pixel:10,20,300,150&t=1.5,4' },
    });
    expect(
      toW3C(draft({ kind: 'polygon', points: [[10, 20], [30, 40], [50, 60]] }), CANVAS)['target'],
    ).toMatchObject({
      selector: {
        type: 'SvgSelector',
        value: '<svg xmlns="http://www.w3.org/2000/svg"><polygon points="10,20 30,40 50,60"/></svg>',
      },
    });
    expect(toW3C(draft({ kind: 'whole' }), CANVAS)['target']).toBe(CANVAS);
  });

  it('keeps the server document it was given, replacing only the edited fields', () => {
    const stored = firstNote(loadFixture('annotation-page.json')).raw;
    const document = toW3C(draft(rect, 'Watermark: a shield'), CANVAS, stored);
    expect(document['dcterms:source']).toBe('Briquet 1442');
    expect(document['id']).toBe('https://archive.example/annotations/90311');
    expect(document['body']).toMatchObject({ type: 'TextualBody', value: 'Watermark: a shield', language: 'en' });
    expect(document['@context']).toBe('http://www.w3.org/ns/anno.jsonld');
  });

  it('keeps a link body beside the edited text body', () => {
    const stored = {
      id: 'https://archive.example/annotations/90311',
      motivation: 'identifying',
      body: [
        { id: 'https://example.org/briquet/1442', purpose: 'identifying' },
        { type: 'TextualBody', value: 'old', format: 'text/plain' },
      ],
      target: CANVAS,
    };
    const bodies = toW3C(draft({ kind: 'whole' }, 'Briquet 1442'), CANVAS, stored)['body'];
    expect(bodies).toEqual([
      { id: 'https://example.org/briquet/1442', purpose: 'identifying' },
      { type: 'TextualBody', value: 'Briquet 1442', format: 'text/plain', language: 'en' },
    ]);
  });
});

describe('parseSurface', () => {
  it('reads the image form', () => {
    expect(parseSurface('image; width=4000; height=3000; service="https://archive.example/iiif/image/3/4127"')).toEqual({
      kind: 'image',
      width: 4000,
      height: 3000,
      service: 'https://archive.example/iiif/image/3/4127',
    });
  });

  it('reads the time form, with and without a duration', () => {
    expect(parseSurface('time; duration=62.5')).toEqual({ kind: 'time', durationMs: 62500, frame: null });
    expect(parseSurface('time')).toEqual({ kind: 'time', durationMs: null, frame: null });
  });

  it("reads a video's frame, the pixels its boxes are drawn in", () => {
    expect(parseSurface('time; duration=90; width=640; height=360')).toEqual({
      kind: 'time',
      durationMs: 90000,
      frame: { width: 640, height: 360 },
    });
  });

  it('reads the whole form', () => {
    expect(parseSurface('whole')).toEqual({ kind: 'whole' });
  });

  it.each([
    ['a missing header', null],
    ['an unknown surface', 'pdf'],
    ['an image with no service', 'image; width=10; height=10'],
    ['a zero size', 'image; width=0; height=10; service="https://archive.example/iiif/image/3/1"'],
    ['a duration that is not a number', 'time; duration=soon'],
  ])('refuses %s', (_name, header) => {
    expect(() => parseSurface(header)).toThrow(ParseError);
  });
});

describe('targetSourceOf', () => {
  it('reads a bare target and a specific resource source', () => {
    expect(targetSourceOf({ target: CANVAS })).toBe(CANVAS);
    expect(targetSourceOf({ target: { source: CANVAS } })).toBe(CANVAS);
    expect(targetSourceOf({ target: { source: { id: CANVAS } } })).toBe(CANVAS);
    expect(targetSourceOf({ body: 'no target' })).toBeNull();
  });
});

describe('the folder boundary', () => {
  it('refuses a file outside the folder that imports wire.ts', () => {
    const eslint = resolve(process.cwd(), 'node_modules', 'eslint', 'bin', 'eslint.js');
    const result = spawnSync(
      process.execPath,
      [eslint, '--no-ignore', '--no-warn-ignored', 'tests/lint/import-annotations-wire.ts'],
      { cwd: process.cwd(), encoding: 'utf8' },
    );
    expect(result.status).toBe(1);
    expect(result.stdout).toContain('no-restricted-imports');
  }, 60_000);
});
