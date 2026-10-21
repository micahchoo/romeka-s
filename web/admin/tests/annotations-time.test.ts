/**
 * The time editor through the notes panel's public interface: a range taken
 * from the player, the two fields, the keys, and the range checks that keep a
 * note the server would refuse from being sent.
 *
 * The panel is the only component driven; the player is a real `<audio>`
 * element, so `currentTime` is the same value the editor reads in a browser.
 */
import { createElement } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import type { Media } from '../src/api/types';
import { MediaNotes } from '../src/pages/media/annotations/panel';
import { clockText, parseClock, rangeProblem } from '../src/pages/media/annotations/time-text';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const MEDIA_ID = 9;
const BASE = 'http://localhost';
const CANVAS = `${BASE}/iiif/3/item/2/canvas/9`;
const CONTAINER_PATH = `/annotations/media/${String(MEDIA_ID)}/`;

const media: Media = {
  id: brandId<'MediaId'>(MEDIA_ID),
  apiUrl: `${BASE}/api/media/9`,
  types: ['o:Media'],
  isPublic: true,
  owner: null,
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  title: null,
  created: '2026-10-20T09:00:00+00:00',
  modified: null,
  thumbnailDisplayUrls: {},
  values: new Map(),
  ingester: 'upload',
  renderer: 'file',
  kind: null,
  item: { id: brandId<'ItemId'>(2), apiUrl: `${BASE}/api/items/2` },
  source: null,
  mediaType: 'audio/mpeg',
  sha256: null,
  size: 1748,
  filename: 'audio.mp3',
  lang: 'en',
  altText: null,
  originalUrl: `${BASE}/files/original/9.mp3`,
  thumbnailUrls: {},
};

interface NoteFacts {
  readonly id: string;
  readonly text: string;
  readonly start: number;
  readonly end: number;
}

const noteDocument = (facts: NoteFacts): Record<string, unknown> => ({
  id: `${BASE}/annotations/${facts.id}`,
  type: 'Annotation',
  motivation: 'commenting',
  modified: '2026-10-20T09:12:44Z',
  body: { type: 'TextualBody', value: facts.text, format: 'text/plain', language: 'en' },
  target: {
    type: 'SpecificResource',
    source: { id: CANVAS, type: 'Canvas' },
    selector: {
      type: 'FragmentSelector',
      conformsTo: 'http://www.w3.org/TR/media-frags/',
      value: `t=${String(facts.start)},${String(facts.end)}`,
    },
  },
});

const containerOf = (
  items: readonly NoteFacts[],
  writable = true,
  surface = 'time; duration=120',
): Response =>
  jsonResponse(
    {
      '@context': ['http://www.w3.org/ns/anno.jsonld', 'http://www.w3.org/ns/ldp.jsonld'],
      id: `${BASE}${CONTAINER_PATH}`,
      type: ['BasicContainer', 'AnnotationCollection'],
      total: items.length,
      first: {
        id: `${BASE}${CONTAINER_PATH}page`,
        type: 'AnnotationPage',
        startIndex: 0,
        items: items.map(noteDocument),
      },
    },
    {
      headers: {
        'Romeka-Surface': surface,
        'Romeka-Canvas': CANVAS,
        Allow: writable ? 'GET, HEAD, POST, OPTIONS' : 'GET, HEAD, OPTIONS',
      },
    },
  );

/** The saved note's selector as the panel posts it. */
const postedRange = (body: Record<string, unknown>): string => {
  const target = body['target'];
  if (typeof target !== 'object' || target === null || !('selector' in target)) {
    throw new Error('the posted note carries no selector');
  }
  const selector = target.selector;
  if (typeof selector !== 'object' || selector === null || !('value' in selector)) {
    throw new Error('the posted selector carries no value');
  }
  return String(selector.value);
};

/** A player at a position, with the two media methods jsdom does not implement. */
const playerAt = (seconds: number): HTMLAudioElement => {
  const element = document.createElement('audio');
  element.currentTime = seconds;
  Object.defineProperty(element, 'pause', { value: () => undefined });
  Object.defineProperty(element, 'play', { value: () => Promise.resolve() });
  return element;
};

const leadField = (name: 'Start' | 'End'): HTMLElement =>
  screen.getByLabelText(name, { exact: true });

beforeEach(() => {
  window.sessionStorage.clear();
});

afterEach(() => {
  cleanup();
  window.sessionStorage.clear();
});

describe('a typed time', () => {
  it('is the same moment in each of the four forms an editor types', () => {
    expect(parseClock('62')).toBe(62_000);
    expect(parseClock('1:02')).toBe(62_000);
    expect(parseClock('1:02.5')).toBe(62_500);
    expect(parseClock('1:02:03')).toBe(3_723_000);
    expect(parseClock('1:02:03.25')).toBe(3_723_250);
  });

  it('refuses a seconds field of sixty or more after another field', () => {
    expect(parseClock('1:75')).toBeNull();
    expect(parseClock('1:02:75')).toBeNull();
    expect(parseClock('')).toBeNull();
    expect(parseClock('one minute')).toBeNull();
  });

  it('is shown back in one decimal, and in hours only when there is one', () => {
    expect(clockText(62_500)).toBe('1:02.5');
    expect(clockText(62_000)).toBe('1:02');
    expect(clockText(1000)).toBe('0:01');
    expect(clockText(3_723_000)).toBe('1:02:03');
  });
});

describe('a range the server would refuse', () => {
  it('has no length, and cannot pass the recording', () => {
    expect(rangeProblem(10_000, 10_000, 120_000)).toBe('order');
    expect(rangeProblem(20_000, 10_000, 120_000)).toBe('order');
    expect(rangeProblem(110_000, 130_000, 120_000)).toBe('pastEnd');
    expect(rangeProblem(10_000, 20_000, 120_000)).toBeNull();
    // A failed probe leaves the length unknown, so only the order is checked.
    expect(rangeProblem(10_000, 900_000, null)).toBeNull();
  });
});

describe('the time editor', () => {
  it('takes the start from the player and five seconds after it', async () => {
    const saved: NoteFacts[] = [];
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === CONTAINER_PATH && (init?.method ?? 'GET') === 'POST') {
        const body = requestBody({ url, init });
        const value = postedRange(body);
        const [start, end] = value.replace('t=', '').split(',').map(Number);
        saved.push({ id: '1', text: 'The speaker names the place', start: start ?? 0, end: end ?? 0 });
        return jsonResponse(noteDocument(saved[0] ?? { id: '1', text: '', start: 0, end: 1 }), {
          status: 201,
          headers: { ETag: '"r1"' },
        });
      }
      return containerOf(saved);
    });

    render(createElement(MediaNotes, { media, player: playerAt(100) }));
    fireEvent.click(await screen.findByRole('button', { name: 'Note from here' }));

    expect(leadField('Start')).toHaveValue('1:40');
    expect(leadField('End')).toHaveValue('1:45');

    fireEvent.change(await screen.findByLabelText('Note'), {
      target: { value: 'The speaker names the place' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await screen.findByText('1 note. 1 at a time, 0 on the whole file.');
    const posts = requestsFor(fetchMock, CONTAINER_PATH).filter(
      (request) => request.init?.method === 'POST',
    );
    expect(posts).toHaveLength(1);
    expect(postedRange(requestBody(posts[0] ?? { url: new URL(BASE), init: undefined }))).toBe(
      't=100,105',
    );
    expect(screen.getByText('1:40 to 1:45')).toBeDefined();
  });

  it('caps that five seconds at the recording’s length', async () => {
    installFetch(() => containerOf([]));

    render(createElement(MediaNotes, { media, player: playerAt(118) }));
    fireEvent.click(await screen.findByRole('button', { name: 'Note from here' }));

    expect(leadField('Start')).toHaveValue('1:58');
    expect(leadField('End')).toHaveValue('2:00');
  });

  it('keeps a note whose end is before its start from being sent, and says why', async () => {
    const fetchMock = installFetch(() => containerOf([]));

    render(createElement(MediaNotes, { media, player: playerAt(30) }));
    fireEvent.click(await screen.findByRole('button', { name: 'Note from here' }));
    fireEvent.change(leadField('End'), { target: { value: '0:10' } });

    expect(await screen.findByText('End must be after the start (0:30).')).toBeDefined();
    fireEvent.change(screen.getByLabelText('Note'), { target: { value: 'Out of order' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, CONTAINER_PATH).filter(
          (request) => request.init?.method === 'POST',
        ),
      ).toHaveLength(0);
    });
  });

  it('sets the end from the `]` key on the focused player', async () => {
    installFetch(() => containerOf([]));

    const player = playerAt(0);
    render(createElement(MediaNotes, { media, player }));
    fireEvent.click(await screen.findByRole('button', { name: 'Note from here' }));

    player.currentTime = 12.5;
    fireEvent.keyDown(player, { key: ']' });
    await waitFor(() => {
      expect(leadField('End')).toHaveValue('0:12.5');
    });
  });

  it('seeks the player when a saved time is opened', async () => {
    const notes: NoteFacts[] = [{ id: '1', text: 'A footstep', start: 62, end: 70.5 }];
    installFetch((url) =>
      url.pathname === '/annotations/1'
        ? jsonResponse(noteDocument(notes[0] ?? { id: '1', text: '', start: 0, end: 1 }), {
            headers: { ETag: '"r1"' },
          })
        : containerOf(notes),
    );

    const player = playerAt(0);
    render(createElement(MediaNotes, { media, player }));
    await screen.findByText('1:02 to 1:10.5');

    const segment = await screen.findByRole('button', { name: '1:02 to 1:10.5, A footstep' });
    fireEvent.click(segment);

    expect(player.currentTime).toBe(62);
    expect(await screen.findByLabelText('Note')).toHaveValue('A footstep');
  });

  it('offers a reader the bar and no control', async () => {
    installFetch(() => containerOf([{ id: '1', text: 'A footstep', start: 62, end: 70.5 }], false));

    render(createElement(MediaNotes, { media, player: playerAt(0) }));
    await screen.findByText('1:02 to 1:10.5');

    expect(screen.queryByRole('button', { name: 'Note from here' })).toBeNull();
    expect(screen.queryByLabelText('Start')).toBeNull();
    expect(
      within(screen.getByRole('group', { name: 'Time notes on the recording' })).getAllByRole(
        'button',
      ),
    ).toHaveLength(1);
  });

  it('shows the length the server states, and says when it knows none', async () => {
    installFetch(() => containerOf([], true, 'time'));
    const first = render(createElement(MediaNotes, { media, player: playerAt(0) }));
    await screen.findByText('Length unknown. The player still plays the file.');
    first.unmount();

    installFetch(() => containerOf([]));
    render(createElement(MediaNotes, { media, player: playerAt(0) }));
    await screen.findByText('2:00');
  });
});
