/**
 * The content blocks' inspectors (gallery, featured item, button, quote,
 * accordion, timeline, search box): each block's form writes the settings
 * its renderer reads, through the same `RomkaForm` every Romeka block uses.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { RomkaForm } from '../src/pages/sites/editor/fields/RomkaForm';
import { romkaFor, settingsOf } from '../src/pages/sites/editor/romeka';
import { apply, blockAt, load } from './editor-draft-helpers';
import { SITE_ID } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const handler = (url: URL): Response => {
  if (url.pathname.startsWith('/api/sites/')) {
    return jsonResponse(loadFixture('site-1.json'));
  }
  switch (url.pathname) {
    case '/api/items':
      return listResponse(loadFixture('items-page.json'), 2);
    case '/api/items/5':
    case '/api/items/12':
      return jsonResponse(loadFixture('item-1.json'));
    case '/api/media':
      return listResponse(loadFixture('media-page.json'), 2);
    case '/api/properties':
      return jsonResponse(loadFixture('properties-page.json'));
    case '/api/item_sets':
      return jsonResponse(loadFixture('item-sets-page.json'));
    case '/api/resource_classes':
      return jsonResponse(loadFixture('resource-classes-page.json'));
    default:
      return jsonResponse(
        { errors: { error: `Unexpected request to ${url.pathname}` } },
        { status: 404 },
      );
  }
};

const Harness = ({ layout }: { readonly layout: string }): ReactElement => {
  const [state, setState] = useState<PageDraft>(() =>
    apply(load([]), { type: 'addBlock', layout, index: 0 }),
  );
  const block = blockAt(state.blocks, 0);
  const romka = romkaFor(BLOCK_REGISTRY, block);
  if (romka === undefined) {
    throw new Error(`the ${layout} carrier was not recognised`);
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
  const parsed: unknown = JSON.parse(screen.getByTestId('settings').textContent);
  return typeof parsed === 'object' && parsed !== null
    ? Object.fromEntries(Object.entries(parsed))
    : {};
};

describe('the gallery block form', () => {
  it('stores each picked item as a record with its media and caption', async () => {
    installFetch(handler);
    render(<Harness layout="gallery" />);

    const pick = await screen.findByRole('option', { name: 'Fixture Item One', selected: false });
    fireEvent.click(pick);
    expect(stored()).toMatchObject({ media: [{ item: 12, media: null, caption: '' }] });

    fireEvent.click(await screen.findByRole('radio', { name: /Fixture photo/ }));
    expect(stored()).toMatchObject({ media: [{ item: 12, media: 7, caption: '' }] });
  });

  it('starts as a three-column grid with captions and the lightbox on', () => {
    installFetch(handler);
    render(<Harness layout="gallery" />);

    expect(screen.getByLabelText('Layout')).toHaveValue('grid');
    expect(screen.getByLabelText('Columns')).toHaveValue(3);
    expect(screen.getByRole('checkbox', { name: 'Show captions' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Open images in a lightbox' })).toBeChecked();
  });

  it('offers the asset list instead of the items for an asset gallery', () => {
    installFetch(handler);
    render(<Harness layout="gallery" />);

    expect(screen.getByRole('group', { name: 'Images' })).toHaveClass('attachments-field');
    fireEvent.change(screen.getByLabelText('Images from'), { target: { value: 'assets' } });
    expect(screen.getByRole('group', { name: 'Images' })).toHaveClass('repeat-field');
    expect(stored()).toMatchObject({ source: 'assets' });
  });
});

describe('the featured item block form', () => {
  it('stores the one picked item, its image side and the chosen fields', async () => {
    installFetch(handler);
    render(<Harness layout="featuredItem" />);

    const pick = await screen.findByRole('option', { name: 'Fixture Item One', selected: false });
    fireEvent.click(pick);
    expect(stored()).toMatchObject({ item: 12 });

    fireEvent.change(screen.getByLabelText('Image'), { target: { value: 'top' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));
    const field = await screen.findByLabelText('Field');
    await screen.findByRole('option', { name: 'Description (dcterms:description)' });
    fireEvent.change(field, { target: { value: 'dcterms:description' } });
    expect(stored()).toMatchObject({
      image: 'top',
      fields: [{ property: 'dcterms:description' }],
    });
  });
});

describe('the button block form', () => {
  it('stores a page or a URL for each button, with the style, alignment and size', () => {
    installFetch(handler);
    render(<Harness layout="button" />);

    fireEvent.change(screen.getByLabelText('Button text'), { target: { value: 'Plan a visit' } });
    fireEvent.change(screen.getByLabelText('Style'), { target: { value: 'outline' } });
    fireEvent.change(screen.getByLabelText('Size'), { target: { value: 'large' } });
    expect(stored()).toMatchObject({ button_label: 'Plan a visit', style: 'outline', size: 'large' });
    expect(screen.getByLabelText('Alignment')).toHaveValue('left');
  });
});

describe('the quote block form', () => {
  it('stores the attribution, the source item and the style', async () => {
    installFetch(handler);
    render(<Harness layout="quote" />);

    expect(screen.getByLabelText('Style')).toHaveValue('large');
    fireEvent.change(screen.getByLabelText('Said by'), { target: { value: 'Ada Lovelace' } });
    fireEvent.change(screen.getByLabelText('Style'), { target: { value: 'band' } });
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));
    expect(stored()).toMatchObject({ attribution: 'Ada Lovelace', style: 'band', item: 12 });
  });
});

describe('the accordion block form', () => {
  it('stores each entry as a heading and an answer', () => {
    installFetch(handler);
    render(<Harness layout="accordion" />);

    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));
    fireEvent.change(screen.getByLabelText('Question or heading'), {
      target: { value: 'When are you open?' },
    });
    expect(stored()).toMatchObject({ entries: [{ heading: 'When are you open?' }] });
  });
});

describe('the timeline block form', () => {
  it('reads dcterms:date unless another date field is chosen', async () => {
    installFetch(handler);
    render(<Harness layout="timeline" />);

    const field = screen.getByLabelText('Date field');
    await screen.findByRole('option', { name: 'Description (dcterms:description)' });
    expect(field).toHaveValue('dcterms:date');
    fireEvent.change(field, { target: { value: 'dcterms:description' } });
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));
    expect(stored()).toMatchObject({ property: 'dcterms:description', items: [12] });
  });
});

describe('the search box block form', () => {
  it('searches the whole site until a resource is chosen', () => {
    installFetch(handler);
    render(<Harness layout="searchBox" />);

    expect(screen.getByLabelText('Search in')).toHaveValue('all');
    fireEvent.change(screen.getByLabelText('Search in'), { target: { value: 'items' } });
    fireEvent.change(screen.getByLabelText('Placeholder'), { target: { value: 'Find a ship' } });
    expect(stored()).toMatchObject({ resource: 'items', placeholder: 'Find a ship' });
  });
});
