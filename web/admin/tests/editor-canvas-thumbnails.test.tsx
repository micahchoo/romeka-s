/**
 * The canvas card thumbnails: a block that names an image shows it, and the
 * canvas reads every card's image in one request per kind, not one per card.
 */
import { render, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { PageCanvas } from '../src/pages/sites/editor/canvas';
import { blockAt, load, rawBlock } from './editor-draft-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, urlsFor } from './stub';

const MEDIA_THUMB = 'http://localhost:8124/application/asset/thumbnails/image.png';
const ASSET_URL = 'http://localhost/files/asset/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.png';

const ref = (kind: string, id: number): Record<string, unknown> => ({
  '@id': `http://localhost/api/${kind}/${String(id)}`,
  'o:id': id,
});

const handler = (url: URL): Response => {
  if (url.pathname === '/api/media') {
    return listResponse([loadFixture('media-951.json')], 1);
  }
  if (url.pathname === '/api/assets') {
    return listResponse(loadFixture('assets-page.json'), 2);
  }
  return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

const cardImage = (key: string): string | null =>
  document.querySelector(`[data-key="${key}"] img`)?.getAttribute('src') ?? null;

describe('canvas card thumbnails', () => {
  it('shows the media and asset images, one request per kind for every card', async () => {
    const fetchMock = installFetch(handler);
    const media = rawBlock('media', {}, {
      'o:attachment': [{ 'o:item': ref('items', 31), 'o:media': ref('media', 951), 'o:caption': '' }],
    });
    const draft = load([
      media,
      media,
      rawBlock('asset', { attachments: [{ id: '1', page: '', alt_link_title: '', caption: '' }] }),
      rawBlock('html', { html: '<p>No image</p>' }),
    ]);
    render(
      <PageCanvas draft={draft} siteSettings={null} selectedKey={null} onSelect={() => {}} dispatch={() => {}} />,
    );

    await waitFor(() => {
      expect(cardImage(blockAt(draft.blocks, 0).key)).toBe(MEDIA_THUMB);
    });
    expect(cardImage(blockAt(draft.blocks, 1).key)).toBe(MEDIA_THUMB);
    expect(cardImage(blockAt(draft.blocks, 2).key)).toBe(ASSET_URL);
    expect(cardImage(blockAt(draft.blocks, 3).key)).toBeNull();

    const mediaReads = urlsFor(fetchMock, '/api/media');
    expect(mediaReads).toHaveLength(1);
    expect(mediaReads[0]?.searchParams.get('id')).toBe('951');
    const assetReads = urlsFor(fetchMock, '/api/assets');
    expect(assetReads).toHaveLength(1);
    expect(assetReads[0]?.searchParams.get('id')).toBe('1');
    expect(urlsFor(fetchMock, '/api/items')).toHaveLength(0);
  });
});
