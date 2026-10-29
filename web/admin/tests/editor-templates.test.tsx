/**
 * The page templates in the editor: the create screen's chooser starts a draft
 * from a built-in template, the picker inserts a template's blocks at the index,
 * and the page settings write a template to the site's `romeka_page_templates`
 * row.
 *
 * Every request is stubbed, so the editor runs through the real client and its
 * parsers without a server.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { SitesPage } from '../src/pages/sites/page';
import { at } from './editor-draft-helpers';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, requestBody, requestsFor } from './stub';

const PAGE = '/api/site_pages/1';
const SITE = '/api/sites/1';

const handlerFor =
  (site: unknown) =>
  (url: URL): Response => {
    if (url.pathname === PAGE) {
      return jsonResponse(loadFixture('site-page-1.json'));
    }
    if (url.pathname === SITE) {
      return jsonResponse(site);
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
    if (url.pathname === '/api/users') {
      return listResponse(loadFixture('users-page.json'), 2);
    }
    if (url.pathname === '/api/properties') {
      return listResponse(loadFixture('properties-page.json'), 2);
    }
    return jsonResponse(
      { errors: { error: `Unexpected request to ${url.pathname}` } },
      { status: 404 },
    );
  };

const handler = handlerFor(loadFixture('site-1.json'));

/** The site fixture with one saved template in its `o:site_settings` row. */
const siteWithTemplate = (): unknown => {
  const site = loadFixture('site-1.json');
  if (!isRecord(site)) {
    throw new Error('expected a site fixture');
  }
  return {
    ...site,
    'o:site_settings': {
      romeka_page_templates: [
        {
          id: 't1',
          name: 'Mine',
          description: 'A description',
          layout: 'normal',
          layout_data: {},
          blocks: [],
        },
      ],
    },
  };
};

/** The id of the first stored template in an `o:site_settings` body. */
const templateIdOf = (settings: unknown): unknown => {
  if (!isRecord(settings) || !Array.isArray(settings.romeka_page_templates)) {
    throw new Error('expected the template list');
  }
  const entry: unknown = settings.romeka_page_templates[0];
  if (!isRecord(entry)) {
    throw new Error('expected a template entry');
  }
  return entry.id;
};

afterEach(() => {
  window.location.hash = '';
});

describe('the template chooser', () => {
  it('opens the chooser for a new page', async () => {
    installFetch(handler);
    render(<SitesPage param="1" rest={['1', 'pages', 'new']} />);

    expect(
      await screen.findByRole('region', { name: 'Start from a template' }),
    ).toBeInTheDocument();
  });

  it('starts a new page from a built-in template', async () => {
    installFetch(handler);
    render(<SitesPage param="1" rest={['1', 'pages', 'new']} />);

    const chooser = await screen.findByRole('region', { name: 'Start from a template' });
    fireEvent.click(within(chooser).getByRole('button', { name: /Exhibit landing/ }));

    // The canvas shows the template's four blocks, the hero first.
    await screen.findByRole('listitem', { name: /Block 1: Hero banner/ });
    expect(document.querySelectorAll('li.canvas-card')).toHaveLength(4);
    expect(screen.getByRole('listitem', { name: /Block 2: Text/ })).toBeInTheDocument();
  });

  it('opens a blank page from the blank choice', async () => {
    installFetch(handler);
    render(<SitesPage param="1" rest={['1', 'pages', 'new']} />);

    const chooser = await screen.findByRole('region', { name: 'Start from a template' });
    fireEvent.click(within(chooser).getByRole('button', { name: 'Start blank' }));

    expect(
      await screen.findByRole('button', { name: 'Add your first block' }),
    ).toBeInTheDocument();
  });
});

describe('inserting a template', () => {
  it('inserts the template blocks at the index', async () => {
    installFetch(handler);
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    await screen.findByRole('listitem', { name: /Block 1: Text/ });

    const insertButtons = screen.getAllByRole('button', {
      name: /Insert block at position/,
    });
    fireEvent.click(at(insertButtons, insertButtons.length - 1));
    const picker = await screen.findByRole('dialog', { name: 'Insert a block or template' });
    fireEvent.click(within(picker).getByRole('button', { name: 'About' }));

    // The two stored blocks plus the About template's three.
    await screen.findByRole('listitem', { name: /Block 3: Page title/ });
    expect(document.querySelectorAll('li.canvas-card')).toHaveLength(5);
  });
});

describe('the site settings template manager', () => {
  const openManager = async (): Promise<HTMLElement> => {
    fireEvent.click(await screen.findByRole('button', { name: 'Manage templates' }));
    return screen.findByRole('dialog', { name: 'Page templates' });
  };

  it('renames a saved template', async () => {
    const fetchMock = installFetch(handlerFor(siteWithTemplate()));
    render(<SitesPage param="1" rest={['1', 'settings']} />);

    const manager = await openManager();
    fireEvent.change(within(manager).getByLabelText('Template name'), {
      target: { value: 'Renamed' },
    });
    fireEvent.click(within(manager).getByRole('button', { name: 'Rename' }));
    await screen.findByText('Saved.');

    const patch = first(
      requestsFor(fetchMock, SITE).filter((request) => request.init?.method === 'PATCH'),
    );
    expect(requestBody(patch)).toEqual({
      'o:site_settings': {
        romeka_page_templates: [
          {
            id: 't1',
            name: 'Renamed',
            description: 'A description',
            layout: 'normal',
            layout_data: {},
            blocks: [],
          },
        ],
      },
    });
  });

  it('deletes a saved template', async () => {
    const fetchMock = installFetch(handlerFor(siteWithTemplate()));
    render(<SitesPage param="1" rest={['1', 'settings']} />);

    const manager = await openManager();
    fireEvent.click(within(manager).getByRole('button', { name: 'Delete' }));
    await screen.findByText('Saved.');

    const patch = first(
      requestsFor(fetchMock, SITE).filter((request) => request.init?.method === 'PATCH'),
    );
    expect(requestBody(patch)).toEqual({
      'o:site_settings': { romeka_page_templates: [] },
    });
  });
});

describe('saving a template', () => {
  it('writes the romeka_page_templates setting row', async () => {
    const fetchMock = installFetch(handler);
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    await screen.findByRole('listitem', { name: /Block 1: Text/ });

    fireEvent.click(screen.getByRole('button', { name: 'Save as template' }));
    const dialog = await screen.findByRole('dialog', { name: 'Save as template' });
    fireEvent.change(within(dialog).getByLabelText('Template name'), {
      target: { value: 'My template' },
    });
    fireEvent.change(within(dialog).getByLabelText('Description'), {
      target: { value: 'A description' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save as template' }));
    await waitFor(() => {
      expect(
        requestsFor(fetchMock, SITE).some((request) => request.init?.method === 'PATCH'),
      ).toBe(true);
    });

    const patch = first(
      requestsFor(fetchMock, SITE).filter((request) => request.init?.method === 'PATCH'),
    );
    const body = requestBody(patch);
    const settings = body['o:site_settings'];
    expect(settings).toMatchObject({
      romeka_page_templates: [
        {
          name: 'My template',
          description: 'A description',
          layout: 'normal',
          layout_data: [],
          blocks: [
            { layout: 'html', data: { html: '<p>First block.</p>' }, layout_data: null },
            { layout: 'pageTitle', data: [], layout_data: null },
          ],
        },
      ],
    });
    expect(templateIdOf(settings)).toMatch(/^t[0-9a-z-]+$/);
  });
});
