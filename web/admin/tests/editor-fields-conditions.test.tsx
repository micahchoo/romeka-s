/**
 * Fields that depend on a sibling, through live forms: a `when` test hides a
 * control until its sibling holds the value, the browse preview's query builder
 * follows the resource type, and the hero's one link control writes a page or a
 * web address, never both.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { RomkaForm } from '../src/pages/sites/editor/fields/RomkaForm';
import { romkaFor, settingsOf } from '../src/pages/sites/editor/romeka';
import { apply, blockAt, load } from './editor-draft-helpers';
import { BlockFormHarness, SITE_ID } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestsFor } from './stub';

const handler = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/items':
      return listResponse([], 7);
    case '/api/item_sets':
      return url.searchParams.get('per_page') === '1'
        ? listResponse([], 3)
        : jsonResponse(loadFixture('item-sets-page.json'));
    case '/api/media':
      return listResponse([], 5);
    case '/api/resource_classes':
      return jsonResponse(loadFixture('resource-classes-page.json'));
    case '/api/site_pages':
      return jsonResponse([loadFixture('site-page-1.json')]);
    default:
      return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  }
};

describe('a field that waits on a sibling', () => {
  it('shows the thumbnail size only for a thumbnail display', () => {
    installFetch(handler);
    render(<BlockFormHarness layout="media" data={{}} />);

    expect(screen.queryByLabelText('Thumbnail size')).toBeNull();
    fireEvent.change(screen.getByLabelText('Show media as'), { target: { value: 'thumbnail' } });
    expect(screen.getByLabelText('Thumbnail size')).toHaveValue('square');
  });

  it('offers page links for a list of sites only once it has a limit', () => {
    installFetch(handler);
    render(<BlockFormHarness layout="listOfSites" data={{ limit: 0 }} />);

    expect(screen.queryByLabelText('Split into pages')).toBeNull();
    fireEvent.change(screen.getByLabelText('How many sites'), { target: { value: '6' } });
    expect(screen.getByLabelText('Split into pages')).not.toBeChecked();
  });
});

describe('the browse preview query builder', () => {
  it('counts item sets, without an item set filter, when the block lists item sets', async () => {
    const fetchMock = installFetch(handler);
    render(
      <BlockFormHarness
        layout="browsePreview"
        data={{ resource_type: 'items', query: 'item_set_id=1', components: [] }}
      />,
    );
    expect(await screen.findByText('7 items in this site match.')).toBeInTheDocument();
    expect(screen.getByLabelText('Item set')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('What to list'), { target: { value: 'item_sets' } });

    expect(await screen.findByText('Item sets in this site that match: 3.')).toBeInTheDocument();
    expect(screen.queryByLabelText('Item set')).toBeNull();
    const counted = requestsFor(fetchMock, '/api/item_sets').filter(
      (request) => request.url.searchParams.get('per_page') === '1',
    );
    expect(counted.at(-1)?.url.searchParams.get('site_id')).toBe(String(SITE_ID));
    expect(counted.at(-1)?.url.searchParams.has('item_set_id')).toBe(false);
  });

  it('counts media when the block lists media', async () => {
    installFetch(handler);
    render(
      <BlockFormHarness
        layout="browsePreview"
        data={{ resource_type: 'media', query: '', components: [] }}
      />,
    );

    expect(await screen.findByText('Media in this site that match: 5.')).toBeInTheDocument();
  });

  it('offers a way to add item sets when the site has none', async () => {
    installFetch((url) => (url.pathname === '/api/item_sets' && url.searchParams.get('per_page') === '1' ? listResponse([], 0) : handler(url)));
    render(
      <BlockFormHarness
        layout="browsePreview"
        data={{ resource_type: 'item_sets', query: '', components: [] }}
      />,
    );

    expect(await screen.findByText('This site has no item sets yet.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Add item sets to this site' })).toHaveAttribute(
      'href',
      `#/sites/${String(SITE_ID)}/resources`,
    );
  });
});

const HeroHarness = ({ settings }: { readonly settings: object }): ReactElement => {
  const [state, setState] = useState<PageDraft>(() => {
    const added = apply(load([]), { type: 'addBlock', layout: 'hero', index: 0 });
    const key = blockAt(added.blocks, 0).key;
    return Object.entries(settings).reduce(
      (draft, [field, value]: [string, string]) =>
        draftReducer(draft, { type: 'setLayoutDataValues', blockKey: key, values: { [`romeka.${field}`]: value } }),
      added,
    );
  });
  const block = blockAt(state.blocks, 0);
  const romka = romkaFor(BLOCK_REGISTRY, block);
  if (romka === undefined) {
    throw new Error('the hero carrier was not recognised');
  }
  return (
    <>
      <RomkaForm
        block={block}
        romka={romka}
        issues={[]}
        siteId={SITE_ID}
        dispatch={(action) => {
          setState((current) => draftReducer(current, action));
        }}
      />
      <output data-testid="settings">{JSON.stringify(settingsOf(block))}</output>
    </>
  );
};

const stored = (): Readonly<Record<string, unknown>> => {
  const text = screen.getByTestId('settings').textContent;
  const parsed: unknown = JSON.parse(text);
  return typeof parsed === 'object' && parsed !== null ? Object.fromEntries(Object.entries(parsed)) : {};
};

describe('the hero button link', () => {
  it('starts on a page, and choosing a web address clears the page', async () => {
    installFetch(handler);
    render(<HeroHarness settings={{ cta_page: '1' }} />);
    const first = within(screen.getByRole('group', { name: 'Button links to' }));

    expect(first.getByRole('radio', { name: 'A page' })).toBeChecked();
    await waitFor(() => {
      expect(first.getByRole('combobox', { name: 'A page' })).toHaveValue('1');
    });

    fireEvent.click(first.getByRole('radio', { name: 'A web address' }));
    fireEvent.change(first.getByRole('textbox', { name: 'A web address' }), {
      target: { value: 'https://example.org/visit' },
    });

    expect(stored()).toMatchObject({ cta_page: '', cta_url: 'https://example.org/visit' });
  });

  it('opens on the web address a block stores', () => {
    installFetch(handler);
    render(<HeroHarness settings={{ cta_url: 'https://example.org' }} />);
    const first = within(screen.getByRole('group', { name: 'Button links to' }));

    expect(first.getByRole('radio', { name: 'A web address' })).toBeChecked();
    expect(first.getByRole('textbox', { name: 'A web address' })).toHaveValue('https://example.org');
  });
});
