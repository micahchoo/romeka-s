/**
 * The item page's reach into its files' notes: each card's count and action,
 * the first file's summary, "All notes in this item", and the file viewer.
 *
 * The item, media and container answers are stubs, so the page runs through the
 * real typed client and the real WAP parser.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { ItemsPage } from '../src/pages/items/page';
import { classRoutes } from './class-routes';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, urlsFor } from './stub';

const BASE = 'http://localhost';

/** A media list entry from the live fixture, named and typed for a card. */
const mediaOf = (index: number, source: string, mediaType: string): Record<string, unknown> => {
  const page = loadFixture('media-page-live.json');
  if (!Array.isArray(page)) {
    throw new Error('the media fixture is not a list');
  }
  const list: readonly unknown[] = page;
  const entry = list[index];
  if (!isRecord(entry)) {
    throw new Error('the media fixture has no entry');
  }
  return { ...entry, 'o:title': null, 'o:source': source, 'o:media_type': mediaType };
};

const canvasOf = (id: number): string => `${BASE}/iiif/3/item/1/canvas/${String(id)}`;

const noteOf = (
  media: number,
  id: string,
  text: string,
  selector: Record<string, unknown> | null,
): Record<string, unknown> => ({
  '@context': 'http://www.w3.org/ns/anno.jsonld',
  id: `${BASE}/annotations/${id}`,
  type: 'Annotation',
  motivation: 'commenting',
  body: { type: 'TextualBody', value: text, format: 'text/plain', language: 'en' },
  target:
    selector === null
      ? canvasOf(media)
      : {
          type: 'SpecificResource',
          source: { id: canvasOf(media), type: 'Canvas' },
          selector: { type: 'FragmentSelector', conformsTo: 'http://www.w3.org/TR/media-frags/', ...selector },
        },
});

const rect = (x: number, y: number): Record<string, unknown> => ({
  value: `xywh=pixel:${String(x)},${String(y)},20,20`,
});

const range = (start: number, end: number): Record<string, unknown> => ({
  value: `t=${String(start)},${String(end)}`,
});

const containerOf = (
  media: number,
  surface: string,
  items: readonly unknown[],
  writable: boolean,
): Response =>
  jsonResponse(
    {
      '@context': ['http://www.w3.org/ns/anno.jsonld', 'http://www.w3.org/ns/ldp.jsonld'],
      id: `${BASE}/annotations/media/${String(media)}/`,
      type: ['BasicContainer', 'AnnotationCollection'],
      total: items.length,
      first: {
        id: `${BASE}/annotations/media/${String(media)}/page`,
        type: 'AnnotationPage',
        startIndex: 0,
        items,
      },
    },
    {
      headers: {
        'Romeka-Surface': surface,
        'Romeka-Canvas': canvasOf(media),
        Allow: writable ? 'GET, HEAD, POST, OPTIONS' : 'GET, HEAD, OPTIONS',
        'Romeka-Track-Ids': '',
      },
    },
  );

const MEDIA = [
  mediaOf(0, 'photo.jpg', 'image/jpeg'),
  mediaOf(1, 'clip.mp4', 'video/mp4'),
  mediaOf(2, 'scan.pdf', 'application/pdf'),
];

const serve = () => {
  return installFetch((url, init) => {
    const path = url.pathname;
    if (path === '/api/items/1') {
      return jsonResponse(loadFixture('item-1.json'));
    }
    if (path === '/api/media') {
      return listResponse(MEDIA, MEDIA.length);
    }
    if (path === '/api/users') {
      return jsonResponse(loadFixture('users-page.json'));
    }
    const classRoute = classRoutes[path];
    if (classRoute !== undefined) {
      return classRoute(url, init);
    }
    if (path === '/api/resource_templates') {
      return jsonResponse(loadFixture('resource-templates-page.json'));
    }
    if (path === '/api/properties') {
      return jsonResponse(loadFixture('properties-page.json'));
    }
    if (path === '/api/item_sets') {
      return jsonResponse(loadFixture('item-sets-page.json'));
    }
    if (path === '/api/data_types') {
      return jsonResponse(loadFixture('data-types-page.json'));
    }
    if (path === '/api/sites') {
      return jsonResponse(loadFixture('sites-page.json'));
    }
    if (path === '/annotations/item/1/counts') {
      return jsonResponse({
        media: {
          '951': { count: 2, writable: true },
          '952': { count: 1, writable: false },
          '953': { count: 0, writable: true },
        },
      });
    }
    if (path === '/annotations/media/951/') {
      return containerOf(
        951,
        'image; width=80; height=120; service="http://localhost/iiif/3/1"',
        [
          noteOf(951, '11', 'A printed stamp.', rect(10, 6)),
          noteOf(951, '12', 'A pencil note at the foot.', rect(4, 80)),
        ],
        true,
      );
    }
    if (path === '/annotations/media/952/') {
      return containerOf(952, 'time; duration=90', [noteOf(952, '21', 'The shield appears.', range(62, 70))], false);
    }
    if (path === '/annotations/media/953/') {
      return containerOf(953, 'whole', [], true);
    }
    return jsonResponse({ errors: { error: `unexpected ${init?.method ?? 'GET'} ${path}` } }, { status: 404 });
  });
};

/** The rendered card at `index`, once all three have rendered. */
const card = async (index: number): Promise<HTMLElement> => {
  await waitFor(() => {
    expect(document.querySelectorAll('.media-card').length).toBe(3);
  });
  const found = document.querySelectorAll<HTMLElement>('.media-card')[index];
  if (found === undefined) {
    throw new Error(`the item rendered no card ${String(index)}`);
  }
  return found;
};

/** The element a selector names, or a failure naming the selector. */
const elementOf = (selector: string): HTMLElement => {
  const found = document.querySelector<HTMLElement>(selector);
  if (found === null) {
    throw new Error(`no element matches ${selector}`);
  }
  return found;
};

beforeEach(() => {
  window.sessionStorage.clear();
  window.location.hash = '';
  serve();
});

afterEach(() => {
  window.sessionStorage.clear();
});

describe('an item page and its files’ notes', () => {
  it('names each file’s count from one counts read and reads only a file with notes', async () => {
    const fetchMock = serve();
    render(<ItemsPage param="1" />);
    const first = await card(0);
    await waitFor(() => {
      expect(within(first).getByText('2 notes')).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(document.querySelectorAll('.item-notes-file').length).toBe(2);
    });

    expect(urlsFor(fetchMock, '/annotations/item/1/counts')).toHaveLength(1);
    const containers = fetchMock.mock.calls
      .map(([input]) => (typeof input === 'string' ? input : input instanceof URL ? input.href : input.url))
      .filter((url) => url.includes('/annotations/media/'));
    expect(containers.some((url) => url.includes('/annotations/media/953/'))).toBe(false);
    expect(containers.filter((url) => url.includes('/annotations/media/951/'))).toHaveLength(1);
    expect(containers.filter((url) => url.includes('/annotations/media/952/'))).toHaveLength(1);
  });

  it('names each file’s count and offers the action its rights allow', async () => {
    render(<ItemsPage param="1" />);
    const first = await card(0);
    const second = await card(1);
    const third = await card(2);

    await waitFor(() => {
      expect(within(first).getByText('2 notes')).toBeInTheDocument();
    });
    const annotate = within(first).getByRole('link', { name: 'Annotate' });
    expect(annotate).toHaveAttribute('href', '#/media/951?notes=add');

    // A file whose container offers no POST shows the notes, not a write action.
    await waitFor(() => {
      expect(within(second).getByText('1 note')).toBeInTheDocument();
    });
    const read = within(second).getByRole('link', { name: 'Read notes' });
    expect(read).toHaveAttribute('href', '#/media/952');
    expect(within(second).queryByRole('link', { name: 'Annotate' })).toBeNull();

    // A file with no notes states no count and hides Annotate behind the count.
    expect(within(third).queryByText(/note/u)).toBeNull();
    expect(within(third).getByRole('button', { name: 'Preview' })).toBeInTheDocument();
  });

  it('summarises the first file and groups every file’s notes', async () => {
    render(<ItemsPage param="1" />);

    expect(await screen.findByRole('heading', { name: 'Notes on photo.jpg' })).toBeInTheDocument();
    expect(screen.getByText('2 notes on this file.')).toBeInTheDocument();
    const summary = elementOf('.item-first-notes');
    expect(summary.textContent).toContain('Region 1');
    expect(summary.textContent).toContain('A printed stamp.');
    expect(summary.textContent).toContain('Region 2');
    expect(within(summary).getAllByRole('link')[0]).toHaveAttribute('href', '#/media/951?note=11');

    expect(
      await screen.findByRole('heading', { name: 'All notes in this item' }),
    ).toBeInTheDocument();
    expect(screen.getByText('2 of 3 files have notes.')).toBeInTheDocument();
    const groups = [...document.querySelectorAll<HTMLElement>('.item-notes-file')];
    expect(groups).toHaveLength(2);
    const photo = groups[0];
    const clip = groups[1];
    if (photo === undefined || clip === undefined) {
      throw new Error('the grouped list rendered no file');
    }
    expect(photo.textContent).toContain('photo.jpg');
    expect(photo.textContent).toContain('Image');
    expect(clip.textContent).toContain('clip.mp4');
    // Times are stated as a range, not a picture's place.
    expect(clip.textContent).toContain('1:02 to 1:10');
    expect(within(clip).getByRole('link', { name: /1:02 to 1:10/u })).toHaveAttribute(
      'href',
      '#/media/952?note=21',
    );
  });

  it('opens the viewer on the card’s file and steps through the item', async () => {
    render(<ItemsPage param="1" />);
    fireEvent.click(within(await card(0)).getByRole('button', { name: 'Preview' }));

    const viewer = await screen.findByRole('dialog');
    expect(within(viewer).getByText('File 1 of 3')).toBeInTheDocument();
    expect(within(viewer).getByRole('heading', { name: 'photo.jpg' })).toBeInTheDocument();
    expect(within(viewer).getByRole('button', { name: 'Previous' })).toBeDisabled();
    expect(within(viewer).getByRole('link', { name: 'Annotate' })).toHaveAttribute(
      'href',
      '#/media/951?notes=add',
    );
    expect(window.location.hash).toBe('#/items/1?preview=951');

    fireEvent.click(within(viewer).getByRole('button', { name: 'Next' }));
    expect(await within(viewer).findByText('File 2 of 3')).toBeInTheDocument();
    expect(within(viewer).getByRole('button', { name: 'Previous' })).toBeEnabled();
    expect(window.location.hash).toBe('#/items/1?preview=952');
    // File 952's container offers no POST, so the viewer offers no write action.
    expect(within(viewer).queryByRole('link', { name: 'Annotate' })).toBeNull();
    expect(within(viewer).getByRole('link', { name: 'Open media page' })).toHaveAttribute(
      'href',
      '#/media/952',
    );

    // The arrow keys walk the same order, without leaving the dialog.
    fireEvent.keyDown(viewer, { key: 'ArrowLeft' });
    expect(await within(viewer).findByText('File 1 of 3')).toBeInTheDocument();
    fireEvent.keyDown(viewer, { key: 'ArrowRight' });
    expect(await within(viewer).findByText('File 2 of 3')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/items/1?preview=952');

    // Esc closes and the keyboard goes back to the card the viewer was on.
    fireEvent.keyDown(viewer, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(document.activeElement).toBe(
      document.querySelector('[data-preview-for="952"]'),
    );
  });

  it('opens the viewer from a shared ?preview= address', async () => {
    window.location.hash = '#/items/1?preview=953';
    render(<ItemsPage param="1" />);

    const viewer = await screen.findByRole('dialog');
    expect(within(viewer).getByText('File 3 of 3')).toBeInTheDocument();
    expect(within(viewer).getByRole('heading', { name: 'scan.pdf' })).toBeInTheDocument();

    fireEvent.click(within(viewer).getByRole('button', { name: 'Close the preview' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(window.location.hash).toBe('#/items/1');
  });
});
