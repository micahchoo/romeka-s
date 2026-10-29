/**
 * The page editor's history and its refused save, through the route with every
 * request stubbed: the History panel lists the versions, a version renders
 * through the preview route and restores as an undoable edit, and a save the
 * server refuses as stale offers their version or a second save of mine.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SitesPage } from '../src/pages/sites/page';
import { editorReads } from './editorStubs';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';
import type { CapturedRequest, Handler } from './stub';

const PAGE = '/api/site_pages/1';
const VERSIONS = '/api/site_pages/1/versions';
const PREVIEW = '/s/my-site/preview';
const XSD = 'http://www.w3.org/2001/XMLSchema#dateTime';

const at = (iso: string): Record<string, string> => ({ '@value': iso, '@type': XSD });

/** The stored page fixture, retitled and saved at `modified`. */
const storedPage = (title: string, modified: string | null): Record<string, unknown> => {
  const page = loadFixture('site-page-1.json');
  if (typeof page !== 'object' || page === null) {
    throw new Error('expected the page fixture to be an object');
  }
  return { ...page, 'o:title': title, 'o:modified': modified === null ? null : at(modified) };
};

const today = new Date().toISOString().slice(0, 10);
const LOADED = '2024-01-02T10:00:00+00:00';
const THEIRS = '2024-01-02T11:00:00+00:00';

const NOTHING = {
  'o:title': null,
  'o:settings_changed': false,
  'o:blocks_added': 0,
  'o:blocks_removed': 0,
  'o:blocks_changed': 0,
  'o:blocks_moved': 0,
  'o:changed_blocks': [],
};

const VERSION_LIST = [
  {
    'o:version': 3,
    'o:created': at(`${today}T14:02:00+00:00`),
    'o:origin': 'save',
    'o:user': { 'o:id': 2, 'o:name': 'Ana' },
    'o:author': 'user',
    'o:block_count': 6,
    'o:changes': {
      ...NOTHING,
      'o:title': { 'o:before': 'Intro', 'o:after': 'Welcome' },
      'o:blocks_added': 1,
      'o:blocks_changed': 2,
      'o:blocks_moved': 2,
      'o:changed_blocks': [
        { 'o:layout': 'html', 'o:layout_data': { romeka: { block: 'hero', v: 1, settings: {} } } },
        { 'o:layout': 'html', 'o:layout_data': null },
      ],
    },
  },
  {
    'o:version': 2,
    'o:created': at(LOADED),
    'o:origin': 'save',
    'o:user': null,
    'o:author': 'deleted_user',
    'o:block_count': 1,
    'o:changes': { ...NOTHING, 'o:settings_changed': true, 'o:blocks_changed': 3 },
  },
  {
    'o:version': 1,
    'o:created': at('2024-01-01T09:00:00+00:00'),
    'o:origin': 'before_history',
    'o:user': null,
    'o:author': 'not_recorded',
    'o:block_count': 1,
    'o:changes': null,
  },
];

const withHandler = (overrides: Readonly<Record<string, Handler>>): ReturnType<typeof installFetch> =>
  installFetch((url, init) => {
    const own = overrides[`${init?.method ?? 'GET'} ${url.pathname}`];
    if (own !== undefined) {
      return own(url, init);
    }
    if (url.pathname === PREVIEW) {
      const body: unknown = typeof init?.body === 'string' ? JSON.parse(init.body) : null;
      const title =
        typeof body === 'object' && body !== null && 'o:title' in body ? String(body['o:title']) : '';
      return new Response(`<html><body><h1>${title}</h1></body></html>`, {
        status: 200,
        headers: { 'Content-Type': 'text/html' },
      });
    }
    if (url.pathname === PAGE) {
      return jsonResponse(storedPage('Welcome', LOADED));
    }
    if (url.pathname === VERSIONS) {
      return jsonResponse(VERSION_LIST);
    }
    return (
      editorReads(url) ??
      jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 })
    );
  });

const renderEditor = async (): Promise<void> => {
  render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
  await screen.findByRole('listitem', { name: /Block 1: Text/ });
};

const puts = (fetchMock: ReturnType<typeof installFetch>): readonly CapturedRequest[] =>
  requestsFor(fetchMock, PAGE).filter((request) => request.init?.method === 'PUT');

const titleField = (): HTMLElement => screen.getByLabelText('Title');

afterEach(() => {
  window.location.hash = '';
});

describe('the page history panel', () => {
  it('lists the versions newest first with when, who, how many blocks and what changed', async () => {
    withHandler({});
    await renderEditor();

    fireEvent.click(screen.getByRole('button', { name: 'History' }));

    const panel = await screen.findByRole('region', { name: 'Page history' });
    const entries = await within(panel).findAllByRole('button', { name: / · / });
    expect(
      entries.map((entry) => Array.from(entry.children, (line) => line.textContent)),
    ).toEqual([
      [
        'Today 2:02 PM · Ana · 6 blocks',
        'Title changed from “Intro” to “Welcome”, 1 block added, ' +
          '2 blocks changed: Hero banner and Text, 2 blocks moved',
      ],
      ['Jan 2, 2024, 10:00 AM · Deleted user · 1 block', 'Page settings changed, 3 blocks changed'],
      ['Jan 1, 2024, 9:00 AM · Author not recorded · 1 block', 'First saved version'],
    ]);
    expect(within(panel).getByText('Live now')).toBeInTheDocument();
  });

  it('says when a save changed nothing, and when older versions are gone', async () => {
    const kept = [
      { ...VERSION_LIST[0], 'o:changes': NOTHING },
      { ...VERSION_LIST[1], 'o:version': 7, 'o:changes': null },
    ];
    withHandler({ [`GET ${VERSIONS}`]: () => jsonResponse(kept) });
    await renderEditor();

    fireEvent.click(screen.getByRole('button', { name: 'History' }));

    const panel = await screen.findByRole('region', { name: 'Page history' });
    const entries = await within(panel).findAllByRole('button', { name: / · / });
    expect(entries.map((entry) => entry.lastElementChild?.textContent)).toEqual([
      'No changes',
      'Older versions are no longer kept',
    ]);
  });

  it('says Created for the version a page create made, with who created it', async () => {
    const created = [
      VERSION_LIST[0],
      {
        'o:version': 1,
        'o:created': at('2024-01-01T09:00:00+00:00'),
        'o:origin': 'create',
        'o:user': { 'o:id': 3, 'o:name': 'Ben' },
        'o:author': 'user',
        'o:block_count': 1,
        'o:changes': null,
      },
    ];
    withHandler({ [`GET ${VERSIONS}`]: () => jsonResponse(created) });
    await renderEditor();

    fireEvent.click(screen.getByRole('button', { name: 'History' }));

    const panel = await screen.findByRole('region', { name: 'Page history' });
    const entries = await within(panel).findAllByRole('button', { name: / · / });
    expect(Array.from(entries[1]?.children ?? [], (line) => line.textContent)).toEqual([
      'Jan 1, 2024, 9:00 AM · Ben · 1 block',
      'Created',
    ]);
  });

  it('shows a version through the preview route and restores it as an undoable edit', async () => {
    const fetchMock = withHandler({
      [`GET ${VERSIONS}/2`]: () => jsonResponse(storedPage('Old title', LOADED)),
      [`PUT ${PAGE}`]: () => jsonResponse(storedPage('Old title', THEIRS)),
    });
    await renderEditor();
    fireEvent.click(screen.getByRole('button', { name: 'History' }));
    fireEvent.click(await screen.findByRole('button', { name: /Deleted user/ }));

    const banner = await screen.findByText('Viewing the version from Jan 2, 2024, 10:00 AM');
    expect(banner).toBeInTheDocument();
    const frame = await screen.findByTitle('Viewing the version from Jan 2, 2024, 10:00 AM');
    expect(frame).toHaveAttribute('sandbox', '');
    expect(frame.getAttribute('srcDoc')).toBe('<html><body><h1>Old title</h1></body></html>');
    const previewTitles = requestsFor(fetchMock, PREVIEW).map((request) => requestBody(request)['o:title']);
    expect(previewTitles).toContain('Old title');
    expect(puts(fetchMock)).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Restore this version' }));
    expect(screen.queryByText(/Viewing the version from/)).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'History' }));
    expect(titleField()).toHaveValue('Old title');
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(titleField()).toHaveValue('Welcome');
    fireEvent.click(screen.getByRole('button', { name: 'Redo' }));
    expect(titleField()).toHaveValue('Old title');

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const saved = requestBody(first(puts(fetchMock)));
    expect(saved['o:title']).toBe('Old title');
    expect(saved['o:modified']).toBe(LOADED);
  });

  it('goes back to the current draft without changing it', async () => {
    withHandler({
      [`GET ${VERSIONS}/2`]: () => jsonResponse(storedPage('Old title', LOADED)),
    });
    await renderEditor();
    fireEvent.click(screen.getByRole('button', { name: 'History' }));
    fireEvent.click(await screen.findByRole('button', { name: /Deleted user/ }));
    await screen.findByText(/Viewing the version from/);

    fireEvent.click(screen.getByRole('button', { name: 'Back to the current draft' }));

    expect(screen.queryByText(/Viewing the version from/)).toBeNull();
    expect(await screen.findAllByTitle('Preview of the page')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'History' }));
    expect(titleField()).toHaveValue('Welcome');
    expect(screen.queryByText('Unsaved changes')).toBeNull();
  });
});

describe('a save refused as stale', () => {
  const stale = (): Response =>
    jsonResponse(
      {
        errors: {
          error: 'Someone saved this page after you opened it.',
          'o:modified': at(THEIRS),
          'o:user': { 'o:id': 2, 'o:name': 'Ana' },
        },
      },
      { status: 409 },
    );

  /** The first PUT is refused, every later one saves. */
  const refusedOnce = (): Handler => {
    let calls = 0;
    return () => {
      calls += 1;
      return calls === 1 ? stale() : jsonResponse(storedPage('Mine', '2024-01-02T12:00:00+00:00'));
    };
  };

  const editTitle = (value: string): void => {
    fireEvent.change(titleField(), { target: { value } });
  };

  it('names who saved since, and Keep mine saves over theirs', async () => {
    const fetchMock = withHandler({ [`PUT ${PAGE}`]: refusedOnce() });
    await renderEditor();
    editTitle('Mine');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const dialog = await screen.findByRole('alertdialog', { name: 'Someone else saved this page' });
    expect(dialog).toHaveTextContent('Saved by Ana at Jan 2, 2024, 11:00 AM since you opened this page');
    expect(await screen.findByText(/Not saved: someone else saved this page first/)).toBeInTheDocument();
    expect(requestBody(first(puts(fetchMock)))['o:modified']).toBe(LOADED);

    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep mine' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    const resent = requestBody(puts(fetchMock)[1] ?? first([]));
    expect(resent['o:title']).toBe('Mine');
    expect(resent['o:modified']).toBe(THEIRS);
  });

  it('Show their version loads theirs into the draft and keeps mine one Undo away', async () => {
    let theirs = false;
    const fetchMock = withHandler({
      [`PUT ${PAGE}`]: () => {
        theirs = true;
        return stale();
      },
      [`GET ${PAGE}`]: () =>
        jsonResponse(theirs ? storedPage('Theirs', THEIRS) : storedPage('Welcome', LOADED)),
    });
    await renderEditor();
    editTitle('Mine');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    const dialog = await screen.findByRole('alertdialog');

    fireEvent.click(within(dialog).getByRole('button', { name: 'Show their version' }));

    await waitFor(() => {
      expect(titleField()).toHaveValue('Theirs');
    });
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(screen.queryByText('Unsaved changes')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(titleField()).toHaveValue('Mine');

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(puts(fetchMock)).toHaveLength(2);
    });
    expect(requestBody(puts(fetchMock)[1] ?? first([]))['o:modified']).toBe(THEIRS);
  });
});
