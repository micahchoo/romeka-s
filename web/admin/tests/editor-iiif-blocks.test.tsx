/**
 * The IIIF blocks' Romeka controls: the viewer height, kept in
 * `o:layout_data.romeka` so upstream's `o:data` stays its own, and the
 * manifest source, which stores an item of this site as the manifest URL this
 * server publishes for it.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { BlockDraft } from '../src/pages/sites/editor/draft';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { BlockFormHarness } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const item = loadFixture('item-1.json');
const manifestOf = (id: number): string =>
  `${window.location.origin}/iiif/3/item/${String(id)}/manifest`;

const handler = (url: URL): Response => {
  if (url.pathname === '/api/items') {
    return listResponse([item], 1);
  }
  if (url.pathname === '/api/items/1') {
    return jsonResponse(item);
  }
  return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

/** Renders a block form and returns the latest block it rendered. */
const renderBlock = (layout: string, data: unknown, extra: Record<string, unknown> = {}) => {
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

describe('the IIIF viewer height', () => {
  it('is stored as a Romeka setting of the upstream block, beside its data', () => {
    const block = renderBlock('iiifPresentation', { manifest_url: 'https://example.org/m.json' });
    fireEvent.change(screen.getByLabelText('Viewer height'), { target: { value: 'full' } });
    expect(block().data).toEqual({ manifest_url: 'https://example.org/m.json' });
    expect(block().layoutData).toEqual({
      romeka: { block: 'iiifPresentation', v: BLOCK_REGISTRY.romekaVersion, settings: { height: 'full' } },
    });
  });

  it('reads a stored height back and keeps the other layout data', () => {
    const block = renderBlock(
      'iiifImage',
      { image_url: 'https://example.org/info.json' },
      { 'o:layout_data': { class: 'wide', romeka: { block: 'iiifImage', v: 2, settings: { height: 'small' } } } },
    );
    const select = screen.getByLabelText('Viewer height');
    expect(select).toHaveValue('small');
    fireEvent.change(select, { target: { value: 'large' } });
    expect(block().layoutData).toEqual({
      class: 'wide',
      romeka: { block: 'iiifImage', v: 2, settings: { height: 'large' } },
    });
  });

  it('leaves no carrier once the height is back to standard', () => {
    const block = renderBlock(
      'iiifImage',
      { image_url: 'https://example.org/info.json' },
      { 'o:layout_data': { class: 'wide', romeka: { block: 'iiifImage', v: 2, settings: { height: 'small' } } } },
    );
    fireEvent.change(screen.getByLabelText('Viewer height'), { target: { value: '' } });
    expect(block().layoutData).toEqual({ class: 'wide' });
    expect(block().data).toEqual({ image_url: 'https://example.org/info.json' });
  });
});

describe('the manifest source', () => {
  it('stores an item of this site as the manifest this server publishes', async () => {
    installFetch(handler);
    const block = renderBlock('iiifPresentation', {});
    fireEvent.click(screen.getByRole('radio', { name: 'An item from this site' }));
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));
    expect(block().data).toEqual({ manifest_url: manifestOf(1) });
    expect(await screen.findByRole('option', { name: 'Fixture Item One', selected: true })).toBeInTheDocument();
  });

  it('reads a stored item manifest back as the item, and a web address as a URL', async () => {
    installFetch(handler);
    renderBlock('iiifPresentation', { manifest_url: manifestOf(1) });
    expect(screen.getByRole('radio', { name: 'An item from this site' })).toBeChecked();
    expect(await screen.findByText('Fixture Item One', { selector: 'strong' })).toBeInTheDocument();
  });

  it('clears the stored URL when the source changes', () => {
    const block = renderBlock('iiifPresentation', { manifest_url: 'https://example.org/m.json' });
    expect(screen.getByRole('radio', { name: 'A manifest URL' })).toBeChecked();
    expect(screen.getByRole('textbox', { name: 'A manifest URL' })).toHaveValue('https://example.org/m.json');
    installFetch(handler);
    fireEvent.click(screen.getByRole('radio', { name: 'An item from this site' }));
    expect(block().data).toEqual({ manifest_url: '' });
  });
});
