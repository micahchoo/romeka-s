/**
 * The Romeka settings of four upstream blocks, through a live block form: each
 * is stored under `o:layout_data.romeka.settings` and leaves upstream's
 * `o:data` as upstream writes it. The asset block's per-slot URL is stored
 * beside its slot, so a reorder moves both.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { BlockDraft } from '../src/pages/sites/editor/draft';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { BlockFormHarness } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const V = BLOCK_REGISTRY.romekaVersion;

const handler = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/properties':
      return jsonResponse(loadFixture('properties-page.json'));
    case '/api/items':
    case '/api/item_sets':
    case '/api/media':
      return listResponse([], 0);
    case '/api/resource_classes':
      return jsonResponse(loadFixture('resource-classes-page.json'));
    case '/api/site_pages':
      return jsonResponse([loadFixture('site-page-1.json')]);
    case '/api/assets':
      return jsonResponse(loadFixture('assets-page.json'));
    default:
      return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  }
};

/** Renders a block form and returns the latest block it rendered. */
const renderBlock = (layout: string, data: unknown, extra: Record<string, unknown> = {}) => {
  installFetch(handler);
  let latest: BlockDraft | null = null;
  render(
    <BlockFormHarness
      layout={layout}
      data={data}
      extra={extra}
      onBlock={(block) => {
        latest = block;
      }}
    />,
  );
  return (): BlockDraft => {
    if (latest === null) {
      throw new Error('the form rendered no block');
    }
    return latest;
  };
};

const carrier = (block: string, settings: Record<string, unknown>) => ({
  romeka: { block, v: V, settings },
});

describe('the browse preview look', () => {
  const data = { resource_type: 'items', query: '', components: ['resource-heading', 'thumbnail'] };

  it('stores the card style and the columns as Romeka settings', () => {
    const block = renderBlock('browsePreview', data);
    fireEvent.change(screen.getByLabelText('Card style'), { target: { value: 'carousel' } });
    fireEvent.change(screen.getByLabelText('Columns'), { target: { value: '3' } });
    expect(block().data).toEqual(data);
    expect(block().layoutData).toEqual(carrier('browsePreview', { cards: 'carousel', columns: '3' }));
  });

  it('shows the theme details checked until the curator chooses', () => {
    const block = renderBlock('browsePreview', data);
    const details = screen.getByRole('group', { name: 'Details under the name' });
    expect(within(details).getByRole('checkbox', { name: 'Type' })).toBeChecked();
    expect(within(details).getByRole('checkbox', { name: 'Its item set' })).not.toBeChecked();
    fireEvent.click(within(details).getByRole('checkbox', { name: 'Creator' }));
    fireEvent.click(within(details).getByRole('checkbox', { name: 'Its item set' }));
    expect(block().layoutData).toEqual(
      carrier('browsePreview', { details: ['type', 'date', 'item_set'] }),
    );
  });

  it('sorts by a property through the stored query', async () => {
    const block = renderBlock('browsePreview', data);
    const sort = screen.getByRole('combobox', { name: 'Sort by' });
    await within(sort).findByRole('option', { name: 'Description (dcterms:description)' });
    fireEvent.change(sort, { target: { value: 'dcterms:description' } });
    expect(block().data).toEqual({ ...data, query: 'sort_by=dcterms%3Adescription' });
    expect(block().layoutData).toBeNull();
  });
});

describe('the asset block', () => {
  const slot = { id: '', page: '', alt_link_title: '', caption: '' };

  it('stores a slot URL beside the slot and keeps the slot upstream’s', () => {
    const block = renderBlock('asset', [slot]);
    fireEvent.click(screen.getByRole('radio', { name: 'A web address' }));
    fireEvent.change(screen.getByRole('textbox', { name: 'A web address' }), {
      target: { value: 'https://example.org/shop' },
    });
    expect(block().data).toEqual([slot]);
    expect(block().layoutData).toEqual(
      carrier('asset', { attachments: [{ url: 'https://example.org/shop' }] }),
    );
  });

  it('moves a slot URL with its slot and drops it with its slot', () => {
    const block = renderBlock(
      'asset',
      { attachments: [{ ...slot, caption: 'First' }, { ...slot, caption: 'Second' }] },
      { 'o:layout_data': carrier('asset', { attachments: [{ url: 'https://example.org/a' }, {}] }) },
    );
    const [moveDown] = screen.getAllByRole('button', { name: /^Move .* down$/ });
    if (moveDown === undefined) {
      throw new Error('no move-down button');
    }
    fireEvent.click(moveDown);
    expect(block().data).toEqual([
      { ...slot, caption: 'Second' },
      { ...slot, caption: 'First' },
    ]);
    expect(block().layoutData).toEqual(
      carrier('asset', { attachments: [{}, { url: 'https://example.org/a' }] }),
    );
    const removeSecond = screen.getAllByRole('button', { name: /^Remove item/ })[1];
    if (removeSecond === undefined) {
      throw new Error('no second remove button');
    }
    fireEvent.click(removeSecond);
    expect(block().layoutData).toEqual({});
  });

  it('reads upstream’s index-keyed slots and saves them as a list', () => {
    const block = renderBlock('asset', { '1': { ...slot, caption: 'Second' }, '0': { ...slot, caption: 'First' } });
    const captions = screen.getAllByRole('textbox', { name: 'Caption' });
    expect(captions.map((input) => (input instanceof HTMLInputElement ? input.value : ''))).toEqual(['First', 'Second']);
    fireEvent.change(captions[1] ?? document.body, { target: { value: 'Last' } });
    expect(block().data).toEqual([
      { ...slot, caption: 'First' },
      { ...slot, caption: 'Last' },
    ]);
  });

  it('offers the columns only for a grid', () => {
    const block = renderBlock('asset', { attachments: [slot] });
    expect(screen.queryByLabelText('Columns')).toBeNull();
    fireEvent.change(screen.getByLabelText('Arrange images'), { target: { value: 'grid' } });
    fireEvent.change(screen.getByLabelText('Columns'), { target: { value: '4' } });
    fireEvent.change(screen.getByLabelText('Image shape'), { target: { value: 'contain' } });
    expect(block().layoutData).toEqual(
      carrier('asset', { arrangement: 'grid', columns: '4', fit: 'contain' }),
    );
  });
});

describe('the list of pages', () => {
  it('offers the columns once the pages show as cards', () => {
    const block = renderBlock('listOfPages', { pagelist: '[]' });
    expect(screen.queryByLabelText('Columns')).toBeNull();
    fireEvent.change(screen.getByLabelText('Show pages as'), { target: { value: 'cards' } });
    fireEvent.change(screen.getByLabelText('Columns'), { target: { value: '3' } });
    expect(block().data).toEqual({ pagelist: '[]' });
    expect(block().layoutData).toEqual(carrier('listOfPages', { display: 'cards', columns: '3' }));
  });
});

describe('the item with metadata fields', () => {
  it('adds, orders and removes the properties it prints', async () => {
    const block = renderBlock('itemWithMetadata', {});
    const fields = screen.getByRole('group', { name: 'Fields to show' });
    expect(within(fields).getByText('No fields chosen.')).toBeInTheDocument();
    const add = within(fields).getByLabelText('Add a field');
    await within(fields).findByRole('option', { name: 'Title (dcterms:title)' });
    fireEvent.change(add, { target: { value: 'dcterms:title' } });
    fireEvent.change(add, { target: { value: 'dcterms:description' } });
    expect(block().layoutData).toEqual(
      carrier('itemWithMetadata', { fields: ['dcterms:title', 'dcterms:description'] }),
    );
    fireEvent.click(within(fields).getByRole('button', { name: 'Move Title (dcterms:title) down' }));
    expect(block().layoutData).toEqual(
      carrier('itemWithMetadata', { fields: ['dcterms:description', 'dcterms:title'] }),
    );
    fireEvent.click(within(fields).getByRole('button', { name: 'Remove Description (dcterms:description)' }));
    fireEvent.change(screen.getByLabelText('Show items as'), { target: { value: 'card' } });
    expect(block().layoutData).toEqual(
      carrier('itemWithMetadata', { fields: ['dcterms:title'], display: 'card' }),
    );
  });
});
