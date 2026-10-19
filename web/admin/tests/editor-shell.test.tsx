/**
 * The page editor shell end to end through the route: the canvas sets the
 * inspector's subject, an edit reaches the live preview body after the
 * debounce, the save PUT carries the draft, an unregistered layout round-trips
 * its data, and a dirty draft guards the route.
 *
 * Every request is stubbed, so the shell runs through the real client and its
 * parsers without a server.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, requestBody, requestsFor } from './stub';

const PAGE = '/api/site_pages/1';
const PREVIEW = '/s/my-site/preview';

/** One stored page with the given blocks; the members `parseSitePage` reads. */
const pageWith = (blocks: readonly unknown[]): Record<string, unknown> => ({
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/site_pages/1',
  '@type': ['o:SitePage'],
  'o:id': 1,
  'o:slug': 'welcome',
  'o:title': 'Welcome',
  'o:is_public': true,
  'o:layout': null,
  'o:layout_data': [],
  'o:block': blocks,
  'o:site': { '@id': 'http://localhost/api/sites/1', 'o:id': 1 },
  'o:created': {
    '@value': '2024-01-02T10:00:00+00:00',
    '@type': 'http://www.w3.org/2001/XMLSchema#dateTime',
  },
  'o:modified': null,
});

const optionsResponse = (): Response =>
  listResponse([loadFixture('site-page-1.json')], 1);

const handler =
  (preview: (url: URL, init: RequestInit | undefined) => Response = () =>
    new Response('<html><body><p>First block.</p></body></html>', {
      status: 200,
      headers: { 'Content-Type': 'text/html' },
    })) =>
  (url: URL, init: RequestInit | undefined): Response => {
    if (url.pathname === PREVIEW) {
      return preview(url, init);
    }
    if (url.pathname === PAGE && init?.method === 'PUT') {
      return jsonResponse(loadFixture('site-page-1.json'));
    }
    if (url.pathname === PAGE) {
      return jsonResponse(loadFixture('site-page-1.json'));
    }
    if (url.pathname === '/api/sites/1') {
      return jsonResponse(loadFixture('site-1.json'));
    }
    if (url.pathname === '/api/site_pages') {
      return optionsResponse();
    }
    if (url.pathname === '/api/item_sets') {
      return listResponse(loadFixture('item-sets-page.json'), 2);
    }
    if (url.pathname === '/api/resource_classes') {
      return listResponse(loadFixture('resource-classes-page.json'), 2);
    }
    return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  };

const renderEditor = async (): Promise<void> => {
  render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
  await screen.findByRole('listitem', { name: /Block 1: HTML/ });
};

const htmlEditor = async (): Promise<HTMLElement> =>
  screen.findByRole('textbox', { name: 'HTML' });

afterEach(() => {
  window.location.hash = '';
});

describe('the page editor shell', () => {
  it('selects a block and shows its form in the inspector', async () => {
    installFetch(handler());
    await renderEditor();

    // No selection shows the page settings.
    expect(screen.getByLabelText('Title')).toHaveValue('Welcome');

    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));

    const inspector = screen.getByRole('region', { name: 'Block inspector' });
    expect(await within(inspector).findByRole('textbox', { name: 'HTML' })).toBeInTheDocument();
  });

  it('heads the inspector with the block layout and position, not its draft key', async () => {
    installFetch(handler());
    await renderEditor();
    const count = screen.getAllByRole('listitem', { name: /^Block \d+: / }).length;

    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));

    const inspector = screen.getByRole('region', { name: 'Block inspector' });
    expect(within(inspector).getByRole('heading', { level: 2 })).toHaveTextContent(
      `HTML, block 1 of ${String(count)}`,
    );
  });

  it('posts the changed draft to the preview route after the debounce', async () => {
    const fetchMock = installFetch(handler());
    await renderEditor();
    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));

    const surface = await htmlEditor();
    surface.innerHTML = '<p>Typed words.</p>';
    fireEvent.input(surface);

    // The opening render goes out at once; the edit follows after the debounce.
    const posts = (): ReturnType<typeof requestsFor> =>
      requestsFor(fetchMock, PREVIEW).filter((request) => request.init?.method === 'POST');
    await waitFor(
      () => {
        expect(posts()).toHaveLength(2);
      },
      { timeout: 2000 },
    );

    const post = first(posts().slice(1));
    const headers = post.init?.headers;
    expect(JSON.stringify(headers)).toContain('application/json');
    expect(post.init?.credentials).toBe('same-origin');
    const body = requestBody(post);
    expect(body['o:id']).toBe(1);
    expect(body['o:title']).toBe('Welcome');
    const blocks = body['o:block'];
    expect(Array.isArray(blocks)).toBe(true);
    if (Array.isArray(blocks)) {
      const firstBlock: unknown = blocks[0];
      expect(firstBlock).toMatchObject({
        'o:layout': 'html',
        'o:data': { html: '<p>Typed words.</p>' },
      });
    }
  });

  it('renders the preview answer in a sandboxed frame without a mode switch', async () => {
    installFetch(handler());
    await renderEditor();
    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));

    // Two frames, so a re-render loads behind the one on screen. Both are
    // script-free: `allow-same-origin` and nothing else.
    const frames = await screen.findAllByTitle('Preview of the page');
    expect(frames).toHaveLength(2);
    for (const frame of frames) {
      expect(frame).toHaveAttribute('sandbox', 'allow-same-origin');
    }
    await waitFor(() => {
      expect(
        frames.some(
          (frame) => frame.getAttribute('srcDoc') === '<html><body><p>First block.</p></body></html>',
        ),
      ).toBe(true);
    });
  });

  it('shows the quiet state when the preview route answers 404', async () => {
    installFetch(
      handler(() => jsonResponse({ errors: { error: 'not found' } }, { status: 404 })),
    );
    await renderEditor();
    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));

    expect(await screen.findByText('Live preview is not available yet.')).toBeInTheDocument();
    expect(screen.queryByTitle('Preview of the page')).toBeNull();
  });

  it('keeps the Outline open while the preview fails, whatever the stored choice', async () => {
    window.localStorage.setItem('romeka.editor.outline', 'closed');
    installFetch(handler(() => jsonResponse({ errors: { error: 'Forbidden' } }, { status: 403 })));
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Forbidden');
    expect(await screen.findByRole('listitem', { name: /Block 1: HTML/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Outline' })).toBeDisabled();
    window.localStorage.removeItem('romeka.editor.outline');
  });

  it('closes the Outline and remembers it while the preview works', async () => {
    installFetch(handler());
    await renderEditor();
    await screen.findAllByTitle('Preview of the page');

    fireEvent.click(screen.getByRole('button', { name: 'Outline' }));

    expect(screen.queryByRole('region', { name: 'Outline' })).toBeNull();
    expect(window.localStorage.getItem('romeka.editor.outline')).toBe('closed');
    window.localStorage.removeItem('romeka.editor.outline');
  });

  it('saves the draft as the write the API takes', async () => {
    const fetchMock = installFetch(handler());
    await renderEditor();
    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));

    const surface = await htmlEditor();
    surface.innerHTML = '<p>Edited.</p>';
    fireEvent.input(surface);
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const put = first(
      requestsFor(fetchMock, PAGE).filter((request) => request.init?.method === 'PUT'),
    );
    expect(requestBody(put)).toEqual({
      'o:site': { 'o:id': 1 },
      'o:title': 'Welcome',
      'o:slug': 'welcome',
      'o:is_public': true,
      'o:layout': null,
      'o:layout_data': [],
      'o:block': [
        {
          'o:layout': 'html',
          'o:data': { html: '<p>Edited.</p>' },
          'o:attachment': [],
        },
        { 'o:layout': 'pageTitle', 'o:data': [], 'o:attachment': [] },
      ],
    });
    // A saved draft is clean, so it stops guarding the route.
    const event = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });

  it('edits an unregistered layout as lossless JSON', async () => {
    const unknown = pageWith([
      {
        'o:layout': 'myModuleLayout',
        'o:data': { anything: [1, 'two'], nested: { ok: true } },
        'o:layout_data': null,
        'o:attachment': [],
      },
    ]);
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === PAGE && init?.method === 'PUT') {
        return jsonResponse(unknown);
      }
      if (url.pathname === PAGE) {
        return jsonResponse(unknown);
      }
      return handler()(url, init);
    });

    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    fireEvent.click(await screen.findByRole('listitem', { name: /Block 1: Unknown/ }));

    const source = screen.getByLabelText('Block data (JSON)');
    expect(source).toHaveValue(JSON.stringify({ anything: [1, 'two'], nested: { ok: true } }, null, 2));

    fireEvent.change(source, {
      target: { value: '{"anything":[1,"two","three"],"nested":{"ok":true},"extra":true}' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await screen.findByText('Saved.');

    const put = first(
      requestsFor(fetchMock, PAGE).filter((request) => request.init?.method === 'PUT'),
    );
    const body = requestBody(put);
    const blocks = body['o:block'];
    if (!Array.isArray(blocks)) {
      throw new Error('expected an o:block list');
    }
    expect(blocks[0]).toEqual({
      'o:layout': 'myModuleLayout',
      'o:data': { anything: [1, 'two', 'three'], nested: { ok: true }, extra: true },
      'o:attachment': [],
    });
  });

  it('shows a typed error and sends nothing when the source field is not valid JSON', async () => {
    const unknown = pageWith([
      { 'o:layout': 'myModuleLayout', 'o:data': { ok: true }, 'o:layout_data': null, 'o:attachment': [] },
    ]);
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === PAGE) {
        return jsonResponse(unknown);
      }
      return handler()(url, init);
    });

    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    fireEvent.click(await screen.findByRole('listitem', { name: /Block 1: Unknown/ }));

    fireEvent.change(screen.getByLabelText('Block data (JSON)'), { target: { value: '{' } });

    expect(await screen.findByRole('alert')).toHaveTextContent('The JSON is invalid:');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await screen.findByText('Saved.');
    const put = first(
      requestsFor(fetchMock, PAGE).filter((request) => request.init?.method === 'PUT'),
    );
    const body = requestBody(put);
    const blocks = body['o:block'];
    if (!Array.isArray(blocks)) {
      throw new Error('expected an o:block list');
    }
    expect(blocks[0]).toMatchObject({ 'o:data': { ok: true } });
  });

  it('jumps to the first issue, focusing the field it names', async () => {
    const withIssue = pageWith([
      { 'o:layout': 'oembed', 'o:data': {}, 'o:layout_data': null, 'o:attachment': [] },
    ]);
    installFetch((url, init) => {
      if (url.pathname === PAGE) {
        return jsonResponse(withIssue);
      }
      return handler()(url, init);
    });

    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    fireEvent.click(await screen.findByRole('button', { name: '1 issue' }));

    const field = await screen.findByLabelText('Media URL');
    await waitFor(() => {
      expect(document.activeElement).toBe(field);
    });
  });

  it('adds the first block from the empty state and selects it', async () => {
    const blank = pageWith([]);
    installFetch((url, init) => {
      if (url.pathname === PAGE) {
        return jsonResponse(blank);
      }
      return handler()(url, init);
    });

    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Add your first block' }));

    const inspector = screen.getByRole('region', { name: 'Block inspector' });
    expect(await within(inspector).findByRole('textbox', { name: 'HTML' })).toBeInTheDocument();
  });

  it('guards a dirty draft against a route change', async () => {
    installFetch(handler());
    window.location.hash = '#/sites/1/pages/1';
    await renderEditor();
    fireEvent.click(screen.getByRole('listitem', { name: /Block 1: HTML/ }));
    const surface = await htmlEditor();
    surface.innerHTML = '<p>Dirty.</p>';
    fireEvent.input(surface);

    const unload = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(unload);
    expect(unload.defaultPrevented).toBe(true);

    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
    window.location.hash = '#/sites/1';
    await waitFor(() => {
      expect(confirm).toHaveBeenCalled();
    });
    expect(window.location.hash).toBe('#/sites/1/pages/1');
    confirm.mockRestore();
  });
});
