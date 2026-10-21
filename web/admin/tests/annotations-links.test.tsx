/**
 * The media page's notes panel reached by URL: `?notes=add` opens a draft and
 * `?note=<id>` opens that note, through the same `beginDraft` and `open` its
 * own list calls. A reader without the write grant gets neither a form.
 */
import { render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { MediaPage } from '../src/pages/media/page';
import { annotateHref, mediaHref, noteHref, notesFocusOf } from '../src/pages/media/annotations/link';
import { previewHref, previewIdOf } from '../src/pages/items/preview/url';
import { brandId } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';

const BASE = 'http://localhost';
const MEDIA = 951;
const CANVAS = `${BASE}/iiif/3/item/1/canvas/951`;
const IMAGE = 'image; width=80; height=120; service="http://localhost/iiif/3/1"';

const mediaDocument = (): unknown => loadFixture('media-951.json');

const noteDocument = (id: string, text: string): Record<string, unknown> => ({
  '@context': 'http://www.w3.org/ns/anno.jsonld',
  id: `${BASE}/annotations/${id}`,
  type: 'Annotation',
  motivation: 'commenting',
  body: { type: 'TextualBody', value: text, format: 'text/plain', language: 'en' },
  target: {
    type: 'SpecificResource',
    source: { id: CANVAS, type: 'Canvas' },
    selector: {
      type: 'FragmentSelector',
      conformsTo: 'http://www.w3.org/TR/media-frags/',
      value: 'xywh=pixel:10,10,20,20',
    },
  },
});

const container = (items: readonly unknown[], writable: boolean): Response =>
  jsonResponse(
    {
      '@context': ['http://www.w3.org/ns/anno.jsonld', 'http://www.w3.org/ns/ldp.jsonld'],
      id: `${BASE}/annotations/media/${String(MEDIA)}/`,
      type: ['BasicContainer', 'AnnotationCollection'],
      total: items.length,
      first: {
        id: `${BASE}/annotations/media/${String(MEDIA)}/page`,
        type: 'AnnotationPage',
        startIndex: 0,
        items,
      },
    },
    {
      headers: {
        'Romeka-Surface': IMAGE,
        'Romeka-Canvas': CANVAS,
        Allow: writable ? 'GET, HEAD, POST, OPTIONS' : 'GET, HEAD, OPTIONS',
        'Romeka-Track-Ids': '',
      },
    },
  );

const serve = (writable: boolean, items: readonly unknown[] = []): void => {
  installFetch((url) => {
    if (url.pathname === '/api/media/951') {
      return jsonResponse(mediaDocument(), { headers: { 'Omeka-S-Version': '4.0.0' } });
    }
    if (url.pathname === `/annotations/media/${String(MEDIA)}/`) {
      return container(items, writable);
    }
    for (const item of items) {
      const id = typeof item === 'object' && item !== null && 'id' in item ? String(item.id) : '';
      if (url.pathname === new URL(id).pathname) {
        return jsonResponse(item, { headers: { ETag: '"r1"' } });
      }
    }
    return jsonResponse({ errors: { error: `unexpected ${url.pathname}` } }, { status: 404 });
  });
};

beforeEach(() => {
  window.sessionStorage.clear();
  window.location.hash = '';
});

afterEach(() => {
  window.sessionStorage.clear();
});

describe('a link into the notes panel', () => {
  it('names the media page, a new note and one note', () => {
    expect(mediaHref(brandId<'MediaId'>(7))).toBe('#/media/7');
    expect(annotateHref(brandId<'MediaId'>(7))).toBe('#/media/7?notes=add');
    expect(noteHref(brandId<'MediaId'>(7), '12')).toBe('#/media/7?note=12');
    expect(notesFocusOf('#/media/7?notes=add')).toEqual({ kind: 'add' });
    expect(notesFocusOf('#/media/7?note=12')).toEqual({ kind: 'note', noteId: '12' });
    expect(notesFocusOf('#/media/7')).toBeNull();
    expect(notesFocusOf('#/media/7?note=twelve')).toBeNull();
  });

  it('names the item page, the viewer and the file it shows', () => {
    expect(previewHref(brandId<'ItemId'>(3), brandId<'MediaId'>(9))).toBe('#/items/3?preview=9');
    expect(previewIdOf('#/items/3?preview=9')).toBe(9);
    expect(previewIdOf('#/items/3')).toBeNull();
    expect(previewIdOf('#/items/3?preview=nine')).toBeNull();
  });

  it('opens a new whole-file note from ?notes=add', async () => {
    serve(true);
    window.location.hash = `#/media/${String(MEDIA)}?notes=add`;
    render(<MediaPage param={String(MEDIA)} />);

    await waitFor(() => {
      expect(document.querySelector('.note-form')).not.toBeNull();
    });
    const form = document.querySelector<HTMLElement>('.note-form');
    if (form === null) {
      throw new Error('the panel rendered no form');
    }
    expect(within(form).getByLabelText('Note')).toHaveValue('');
  });

  it('opens the note ?note= names, with its region selected', async () => {
    const note = noteDocument('90210', 'A printed stamp.');
    serve(true, [note]);
    window.location.hash = `#/media/${String(MEDIA)}?note=90210`;
    render(<MediaPage param={String(MEDIA)} />);

    const textarea = await screen.findByLabelText('Note');
    expect(textarea).toHaveValue('A printed stamp.');
    const row = document.querySelector('.note-entry.is-selected');
    expect(row).not.toBeNull();
    expect(row?.textContent).toContain('Region 1');
    await waitFor(() => {
      expect(document.querySelector('.region-shapes .is-selected')).not.toBeNull();
    });
  });

  it('gives a reader no form for ?notes=add', async () => {
    serve(false);
    window.location.hash = `#/media/${String(MEDIA)}?notes=add`;
    render(<MediaPage param={String(MEDIA)} />);

    expect(
      await screen.findByText(
        'You can read these notes. Changing them needs edit rights on this media.',
      ),
    ).toBeInTheDocument();
    expect(document.querySelector('.note-form')).toBeNull();
    expect(document.querySelector('.media-notes textarea')).toBeNull();
  });
});
