/**
 * The site header and footer in the page editor: selecting a region opens the
 * site-wide inspector with the Theme page's controls under its banner, an
 * edit reaches the preview as an `o:theme_settings` override, the banner's
 * Save PATCHes the site's theme settings apart from the page, and Escape or
 * the crumb return to the page settings.
 *
 * jsdom loads no `srcdoc`, so these select through the page settings, the
 * keyboard path to the same selection; the frame map's part is in
 * `editor-frame.test.ts` and the click in the preview is in the e2e spec.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestBody, requestsFor } from './stub';

const PAGE = '/api/site_pages/1';
const SITE = '/api/sites/1';
const PREVIEW = '/s/my-site/preview';

/** The stored theme: a preset, a contact line and a resource page order the region save must keep. */
const STORED_THEME = {
  preset: 'journal',
  footer_contact: '<p>12 Archive Lane</p>',
  resource_page_blocks: { items: { main: ['values', 'mediaList'] } },
};

const siteWith = (themeSettings: unknown): Record<string, unknown> => {
  const site = loadFixture('site-1.json');
  return { ...(typeof site === 'object' && site !== null ? site : {}), 'o:theme_settings': themeSettings };
};

const handler = (url: URL, init: RequestInit | undefined): Response => {
  if (url.pathname === PREVIEW) {
    return new Response('<html><body><p>Preview.</p></body></html>', {
      status: 200,
      headers: { 'Content-Type': 'text/html' },
    });
  }
  if (url.pathname === SITE && init?.method === 'PATCH') {
    const body: unknown = JSON.parse(typeof init.body === 'string' ? init.body : '{}');
    const settings = typeof body === 'object' && body !== null && 'o:theme_settings' in body ? body['o:theme_settings'] : null;
    return jsonResponse(siteWith(settings));
  }
  if (url.pathname === SITE) {
    return jsonResponse(siteWith(STORED_THEME));
  }
  if (url.pathname === PAGE) {
    return jsonResponse(loadFixture('site-page-1.json'));
  }
  if (url.pathname === '/api/site_pages') {
    return listResponse([loadFixture('site-page-1.json')], 1);
  }
  if (url.pathname === '/api/item_sets') {
    return listResponse(loadFixture('item-sets-page.json'), 2);
  }
  if (url.pathname === '/api/resource_classes') {
    return listResponse(loadFixture('resource-classes-page.json'), 2);
  }
  if (url.pathname === '/api/romeka/classes/counts') {
    return jsonResponse({ classes: [] });
  }
  return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

const renderEditor = async (): Promise<void> => {
  render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
  await screen.findByRole('listitem', { name: /Block 1: Text/ });
};

const inspector = (): HTMLElement => screen.getByRole('region', { name: 'Block inspector' });

const openFooter = async (): Promise<void> => {
  fireEvent.click(within(inspector()).getByRole('button', { name: 'Site footer' }));
  await within(inspector()).findByText('Site-wide. Changes every page.');
};

const addFooterColumn = async (html: string): Promise<void> => {
  fireEvent.click(await within(inspector()).findByRole('button', { name: 'Add a column' }));
  const column = await within(inspector()).findByRole('textbox', { name: 'Column 1' });
  column.innerHTML = html;
  fireEvent.input(column);
};

afterEach(() => {
  window.location.hash = '';
});


// The page loads this module lazily. A test that leaves before the import settles
// tears the environment down under it, and vitest then fails the whole run, so the
// module is loaded once here and every lazy() resolves from the module cache.
beforeAll(async () => {
  await import('../src/pages/sites/editor/shell/SiteRegionInspector');
});

describe('the site regions in the page editor', () => {
  it('opens the footer under the site-wide banner, with a crumb and a link to the Theme page', async () => {
    installFetch(handler);
    await renderEditor();

    await openFooter();

    const crumb = within(inspector()).getByRole('navigation', { name: 'Selection' });
    expect(crumb).toHaveTextContent('Page›Site footer');
    expect(within(inspector()).getByRole('heading', { level: 2 })).toHaveTextContent('Site footer');
    expect(within(inspector()).getByRole('link', { name: 'Open the Theme page' })).toHaveAttribute(
      'href',
      '#/sites/1/theme',
    );
    expect(within(inspector()).getByRole('button', { name: 'Save site-wide changes' })).toBeDisabled();
    expect(within(inspector()).getByRole('combobox', { name: 'Footer colour' })).toBeInTheDocument();
  });

  it('opens the header with the logo, banner and header style controls', async () => {
    installFetch(handler);
    await renderEditor();

    fireEvent.click(within(inspector()).getByRole('button', { name: 'Site header' }));

    expect(await within(inspector()).findByRole('textbox', { name: 'Banner text' })).toBeInTheDocument();
    expect(within(inspector()).getByRole('combobox', { name: 'Header style' })).toBeInTheDocument();
    expect(within(inspector()).getByRole('navigation', { name: 'Selection' })).toHaveTextContent(
      'Page›Site header',
    );
  });

  it('returns to the page settings on Escape and on the crumb', async () => {
    installFetch(handler);
    await renderEditor();

    await openFooter();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(await within(inspector()).findByLabelText('Title')).toHaveValue('Welcome');

    await openFooter();
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Page' }));
    expect(await within(inspector()).findByLabelText('Title')).toHaveValue('Welcome');
  });

  it('previews a footer edit as a theme override and saves it apart from the page', async () => {
    const fetchMock = installFetch(handler);
    await renderEditor();
    await openFooter();

    await addFooterColumn('<p>Open Tuesdays</p>');

    const previews = (): ReturnType<typeof requestsFor> =>
      requestsFor(fetchMock, PREVIEW).filter((request) => request.init?.method === 'POST');
    await waitFor(
      () => {
        const last = previews().at(-1);
        expect(last === undefined ? null : requestBody(last)['o:theme_settings']).toMatchObject({
          footer_columns: ['<p>Open Tuesdays</p>'],
        });
      },
      { timeout: 2000 },
    );

    fireEvent.click(within(inspector()).getByRole('button', { name: 'Save site-wide changes' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, SITE).filter((request) => request.init?.method === 'PATCH')).toHaveLength(1);
    });
    const patch = requestsFor(fetchMock, SITE).find((request) => request.init?.method === 'PATCH');
    const settings = patch === undefined ? null : requestBody(patch)['o:theme_settings'];
    expect(settings).toMatchObject({
      preset: 'journal',
      footer_columns: ['<p>Open Tuesdays</p>'],
      footer_contact: '<p>12 Archive Lane</p>',
      resource_page_blocks: { items: { main: ['values', 'mediaList'] } },
    });
    expect(requestsFor(fetchMock, PAGE).filter((request) => request.init?.method === 'PUT')).toHaveLength(0);
    await waitFor(() => {
      expect(within(inspector()).getByRole('button', { name: 'Save site-wide changes' })).toBeDisabled();
    });
  });

  it('keeps an unsaved site-wide edit after a return to the page, and offers the way back', async () => {
    installFetch(handler);
    await renderEditor();
    await openFooter();
    await addFooterColumn('<p>Kept</p>');

    fireEvent.click(within(inspector()).getByRole('button', { name: 'Page' }));
    const back = await screen.findByRole('button', { name: 'Site footer: site-wide changes not saved' });
    fireEvent.click(back);

    expect(await within(inspector()).findByRole('textbox', { name: 'Column 1' })).toHaveTextContent('Kept');
    expect(within(inspector()).getByRole('button', { name: 'Save site-wide changes' })).toBeEnabled();
  });
});
