/**
 * The notes panel through its public interface: the list's order and counts,
 * the read-only surface, the form's draft across a remount, and the two ways a
 * colleague's write meets this editor (412 on save, 412 on delete).
 *
 * Every case drives the real client with stubbed HTTP answers, so the request
 * headers are part of the behavior asserted.
 */
import { createElement } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import type { Media } from '../src/api/types';
import { MediaNotes } from '../src/pages/media/annotations/panel';
import { installFetch, jsonResponse, requestsFor } from './stub';

const MEDIA_ID = 7;
const BASE = 'http://localhost';
const CANVAS = `${BASE}/iiif/3/item/1/canvas/7`;
const CONTAINER_PATH = `/annotations/media/${String(MEDIA_ID)}/`;
const NOTE_PATH = '/annotations/90311';
const IMAGE = 'image; width=4000; height=3000; service="x"';

const media: Media = {
  id: brandId<'MediaId'>(MEDIA_ID),
  apiUrl: `${BASE}/api/media/7`,
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
  item: { id: brandId<'ItemId'>(1), apiUrl: `${BASE}/api/items/1` },
  source: null,
  mediaType: 'application/pdf',
  sha256: null,
  size: 1024,
  filename: 'doc.pdf',
  lang: 'en',
  altText: null,
  originalUrl: `${BASE}/files/original/7.pdf`,
  thumbnailUrls: {},
};

type Selector =
  | { readonly kind: 'whole' }
  | { readonly kind: 'rect'; readonly x: number; readonly y: number }
  | { readonly kind: 'time'; readonly start: number; readonly end: number };

interface NoteFacts {
  readonly id: string;
  readonly text: string;
  readonly selector?: Selector;
  readonly modified?: string;
}

const targetOf = (selector: Selector): unknown => {
  switch (selector.kind) {
    case 'whole':
      return CANVAS;
    case 'rect':
      return {
        type: 'SpecificResource',
        source: { id: CANVAS, type: 'Canvas' },
        selector: {
          type: 'FragmentSelector',
          conformsTo: 'http://www.w3.org/TR/media-frags/',
          value: `xywh=pixel:${String(selector.x)},${String(selector.y)},10,10`,
        },
      };
    case 'time':
      return {
        type: 'SpecificResource',
        source: { id: CANVAS, type: 'Canvas' },
        selector: {
          type: 'FragmentSelector',
          conformsTo: 'http://www.w3.org/TR/media-frags/',
          value: `t=${String(selector.start)},${String(selector.end)}`,
        },
      };
  }
};

const noteDocument = (facts: NoteFacts): Record<string, unknown> => ({
  id: `${BASE}/annotations/${facts.id}`,
  type: 'Annotation',
  motivation: 'commenting',
  modified: facts.modified ?? '2026-10-20T09:12:44Z',
  body: { type: 'TextualBody', value: facts.text, format: 'text/plain', language: 'en' },
  target: targetOf(facts.selector ?? { kind: 'whole' }),
});

interface ContainerOptions {
  readonly surface: string;
  readonly writable?: boolean;
}

/** A container answer around raw annotation documents. */
const containerOf = (
  items: readonly unknown[],
  options: ContainerOptions,
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
        items,
      },
    },
    {
      headers: {
        'Romeka-Surface': options.surface,
        'Romeka-Canvas': CANVAS,
        Allow: options.writable === false ? 'GET, HEAD, OPTIONS' : 'GET, HEAD, POST, OPTIONS',
      },
    },
  );

const answerOf = (notes: readonly NoteFacts[], options: ContainerOptions): Response =>
  containerOf(notes.map(noteDocument), options);

const whole = (id: string, text: string): NoteFacts => ({ id, text, selector: { kind: 'whole' } });

const rect = (id: string, text: string, x: number, y: number): NoteFacts => ({
  id,
  text,
  selector: { kind: 'rect', x, y },
});

const timed = (id: string, text: string, start: number, end: number): NoteFacts => ({
  id,
  text,
  selector: { kind: 'time', start, end },
});

/** One entry per note, in the rendered order. */
const entries = (): readonly HTMLElement[] => {
  const list = screen.queryByRole('list');
  return list === null
    ? []
    : within(list)
        .queryAllByRole('listitem')
        .flatMap((item) => within(item).queryAllByRole('button').slice(0, 1));
};

const openFirst = async (): Promise<void> => {
  const found = await waitFor(() => {
    const rows = entries();
    expect(rows.length).toBeGreaterThan(0);
    return rows;
  });
  const first = found[0];
  if (first === undefined) {
    throw new Error('the list rendered no entry');
  }
  fireEvent.click(first);
};

beforeEach(() => {
  window.sessionStorage.clear();
});

afterEach(() => {
  cleanup();
  window.sessionStorage.clear();
});

describe('the note list', () => {
  it('orders regions top to bottom then left to right, and numbers them', async () => {
    const notes = [
      rect('1', 'Right of the top row', 900, 100),
      rect('2', 'Left of the top row', 200, 100),
      rect('3', 'The second row', 50, 500),
      whole('4', 'The whole file'),
    ];
    installFetch(() => answerOf(notes, { surface: IMAGE }));

    render(createElement(MediaNotes, { media }));
    await waitFor(() => {
      expect(entries()).toHaveLength(4);
    });

    const rows = entries().map((button) => button.textContent);
    expect(rows[0]).toContain('Region 1');
    expect(rows[0]).toContain('Left of the top row');
    expect(rows[1]).toContain('Region 2');
    expect(rows[1]).toContain('Right of the top row');
    expect(rows[2]).toContain('Region 3');
    expect(rows[2]).toContain('The second row');
    expect(rows[3]).toContain('Whole image');
    expect(rows[3]).toContain('The whole file');
  });

  it('orders times by their start and names each range', async () => {
    const notes = [timed('1', 'Second', 62, 70.5), timed('2', 'First', 1, 2)];
    installFetch(() => answerOf(notes, { surface: 'time; duration=120' }));

    render(createElement(MediaNotes, { media }));
    await waitFor(() => {
      expect(entries()).toHaveLength(2);
    });

    const rows = entries().map((button) => button.textContent);
    expect(rows[0]).toContain('0:01 to 0:02');
    expect(rows[0]).toContain('First');
    expect(rows[1]).toContain('1:02 to 1:10.5');
    expect(rows[1]).toContain('Second');
  });

  it('counts what the filter keeps and states the total', async () => {
    const notes: NoteFacts[] = [];
    for (let index = 0; index < 20; index += 1) {
      notes.push(
        rect(
          String(index),
          index < 2 ? `A shield ${String(index)}` : `A region ${String(index)}`,
          index * 10,
          0,
        ),
      );
    }
    for (let index = 0; index < 4; index += 1) {
      notes.push(
        whole(String(100 + index), index === 0 ? 'A shield on the page' : `A whole note ${String(index)}`),
      );
    }
    installFetch(() => answerOf(notes, { surface: IMAGE }));

    render(createElement(MediaNotes, { media }));
    await screen.findByText('24 notes. 20 on regions, 4 on the whole image.');

    fireEvent.click(screen.getByRole('button', { name: 'Whole file' }));
    expect(screen.getByText('Showing 4 of 24 notes')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Filter by text'), { target: { value: 'shield' } });
    expect(screen.getByText('1 of 24 notes match “shield”.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Clear filter' }));
    expect(screen.queryByText(/Showing/)).toBeNull();
    expect(screen.getByText('24 notes. 20 on regions, 4 on the whole image.')).toBeInTheDocument();
    expect(screen.getByLabelText('Filter by text')).toHaveValue('');
  });

  it('shows 50 notes and offers the rest', async () => {
    const notes: NoteFacts[] = [];
    for (let index = 0; index < 51; index += 1) {
      notes.push(rect(String(index), `Region ${String(index)}`, index * 10, 0));
    }
    installFetch(() => answerOf(notes, { surface: IMAGE }));

    render(createElement(MediaNotes, { media }));
    await screen.findByText('Showing 50 of 51 notes');
    expect(entries()).toHaveLength(50);

    fireEvent.click(screen.getByRole('button', { name: 'Show 50 more' }));
    expect(entries()).toHaveLength(51);
    expect(screen.queryByText('Showing 50 of 51 notes')).toBeNull();
  });

  it('says how many notes a file has, in the singular and the plural', async () => {
    installFetch(() => answerOf([whole('1', 'Only note')], { surface: 'whole' }));

    const first = render(createElement(MediaNotes, { media }));
    await screen.findByText('1 note on this file.');
    first.unmount();

    installFetch(() =>
      answerOf([whole('1', 'One'), whole('2', 'Two')], { surface: 'whole' }),
    );
    render(createElement(MediaNotes, { media }));
    await screen.findByText('2 notes on this file.');
  });
});

describe('a reader without the write grant', () => {
  it('lists the notes, opens their text and holds no write control', async () => {
    installFetch(() =>
      answerOf([whole('1', 'Foxed throughout, and the second line stays readable.')], {
        surface: 'whole',
        writable: false,
      }),
    );

    render(createElement(MediaNotes, { media }));
    await screen.findByText(
      'You can read these notes. Changing them needs edit rights on this media.',
    );

    expect(screen.queryByRole('button', { name: 'Add a note about this file' })).toBeNull();
    expect(screen.queryByRole('textbox')).toBeNull();

    await openFirst();
    // The row's snippet and the opened text, and still no way to change either.
    expect(
      screen.getAllByText('Foxed throughout, and the second line stays readable.'),
    ).toHaveLength(2);
    expect(screen.queryByRole('textbox')).toBeNull();
  });
});

describe('an unsaved draft', () => {
  it('comes back after the panel remounts, and can be kept or discarded', async () => {
    installFetch(() => answerOf([], { surface: 'whole' }));

    const first = render(createElement(MediaNotes, { media }));
    fireEvent.click(await screen.findByRole('button', { name: 'Add a note about this file' }));
    fireEvent.change(await screen.findByLabelText('Note'), { target: { value: 'Half a thought' } });
    first.unmount();

    render(createElement(MediaNotes, { media }));
    await screen.findByText('You have an unsaved note. Keep it, or discard it.');
    fireEvent.click(screen.getByRole('button', { name: 'Keep it' }));
    expect(await screen.findByLabelText('Note')).toHaveValue('Half a thought');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
    await waitFor(() => {
      expect(screen.queryByText('You have an unsaved note. Keep it, or discard it.')).toBeNull();
    });
  });
});

describe('a colleague who wrote first', () => {
  it('turns a refused save into both texts and saves this editor’s text over theirs', async () => {
    let current = noteDocument({ id: '90311', text: 'The first text' });
    const fetchMock = installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf([current], { surface: 'whole' });
      }
      if (url.pathname === NOTE_PATH && method === 'GET') {
        return jsonResponse(current, { headers: { ETag: '"r1"' } });
      }
      if (url.pathname === NOTE_PATH && method === 'PUT') {
        const ifMatch = new Headers(init?.headers).get('If-Match');
        if (ifMatch === '"r1"') {
          current = noteDocument({ id: '90311', text: 'What the colleague wrote' });
          return jsonResponse(current, { status: 412, headers: { ETag: '"r2"' } });
        }
        current = noteDocument({ id: '90311', text: 'Mine, after theirs' });
        return jsonResponse(current, { status: 200, headers: { ETag: '"r3"' } });
      }
      return jsonResponse({ errors: { error: `unexpected ${method} ${url.pathname}` } }, { status: 500 });
    });

    render(createElement(MediaNotes, { media }));
    await openFirst();
    fireEvent.change(await screen.findByLabelText('Note'), {
      target: { value: 'Mine, after theirs' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await screen.findByText(
      'Someone else changed this note. Their version is shown. Your text is below it.',
    );
    // The row now shows their version, and the conflict block states it in full.
    expect(screen.getAllByText('What the colleague wrote')).toHaveLength(2);
    expect(screen.getByLabelText('Your text')).toHaveValue('Mine, after theirs');

    fireEvent.click(screen.getByRole('button', { name: 'Save mine over theirs' }));
    await waitFor(() => {
      expect(screen.queryByText('Conflict')).toBeNull();
    });
    expect(entries()[0]?.textContent).toContain('Mine, after theirs');

    const puts = requestsFor(fetchMock, NOTE_PATH).filter(
      (request) => request.init?.method === 'PUT',
    );
    expect(puts).toHaveLength(2);
    expect(new Headers(puts[1]?.init?.headers).get('If-Match')).toBe('"r2"');
  });

  it('shows their version before this editor deletes it', async () => {
    let deleted = false;
    const fetchMock = installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf(
          deleted ? [] : [noteDocument({ id: '90311', text: 'The first text' })],
          { surface: 'whole' },
        );
      }
      if (url.pathname === NOTE_PATH && method === 'GET') {
        return jsonResponse(noteDocument({ id: '90311', text: 'The first text' }), {
          headers: { ETag: '"r1"' },
        });
      }
      if (url.pathname === NOTE_PATH && method === 'DELETE') {
        const ifMatch = new Headers(init?.headers).get('If-Match');
        if (ifMatch === '"r1"') {
          return jsonResponse(noteDocument({ id: '90311', text: 'The colleague’s text' }), {
            status: 412,
            headers: { ETag: '"r2"' },
          });
        }
        deleted = true;
        return new Response(null, { status: 204 });
      }
      return jsonResponse({ errors: { error: `unexpected ${method}` } }, { status: 500 });
    });

    render(createElement(MediaNotes, { media }));
    await openFirst();
    fireEvent.click(await screen.findByRole('button', { name: 'Delete' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete this note' }));

    await screen.findByText(
      'Someone changed this note after you opened it. Their version is shown.',
    );
    expect(screen.getAllByText('The colleague’s text')).toHaveLength(2);

    fireEvent.click(screen.getByRole('button', { name: 'Delete their version' }));
    await screen.findByText('Deleted.');
    await waitFor(() => {
      expect(entries()).toHaveLength(0);
    });

    const deletes = requestsFor(fetchMock, NOTE_PATH).filter(
      (request) => request.init?.method === 'DELETE',
    );
    expect(new Headers(deletes[1]?.init?.headers).get('If-Match')).toBe('"r2"');
  });
});
