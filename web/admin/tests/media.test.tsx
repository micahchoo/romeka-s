/**
 * Media page tests. `media-page-live.json` and `media-951.json` are captures of
 * the running port, where `o:thumbnail_urls` is the empty PHP array and no
 * media has a title. The stubbed fetch serves them through the real client.
 */
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ApiClient } from '../src/api/client';
import { mediaIdFromParam } from '../src/api/parse';
import { MediaPage } from '../src/pages/media/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, route, urlsFor } from './stub';

const ORIGINAL =
  'http://localhost:8124/files/original/e82cef00da75e9c5661a9ee66155ef419cf89988.jpg';
const SQUARE_THUMBNAIL = 'http://localhost:8124/files/thumbnails/square/abcdef0123.jpg';

const listRoutes = {
  '/api/media': () => listResponse(loadFixture('media-page-live.json'), 3),
};

describe('media list', () => {
  it('media_browse_renders_the_original_when_the_row_has_no_thumbnail', async () => {
    const fetchMock = installFetch(route(listRoutes));
    render(<MediaPage param={null} />);

    const images = await screen.findAllByRole('img');
    expect(images).toHaveLength(2);

    const original = first(images);
    expect(original).toHaveAttribute('src', ORIGINAL);
    expect(original).toHaveAttribute('loading', 'lazy');
    expect(original).toHaveAttribute('alt', 'BHC006_GMT_AF_010a.jpg');

    // Untitled media shows its source and links to the media detail route.
    expect(screen.getByRole('link', { name: 'BHC006_GMT_AF_010a.jpg' })).toHaveAttribute(
      'href',
      '#/media/951',
    );

    const mediaUrl = first(urlsFor(fetchMock, '/api/media'));
    expect(mediaUrl.searchParams.get('per_page')).toBe('25');
    expect(mediaUrl.searchParams.get('sort_by')).toBe('id');
    expect(mediaUrl.searchParams.get('sort_order')).toBe('asc');
  });

  it('media_browse_prefers_a_thumbnail_over_the_original', async () => {
    installFetch(route(listRoutes));
    render(<MediaPage param={null} />);

    const images = await screen.findAllByRole('img');
    expect(images).toHaveLength(2);
    expect(first(images)).toHaveAttribute('src', ORIGINAL);
    expect(images[1]).toHaveAttribute('src', SQUARE_THUMBNAIL);
    expect(images[1]).toHaveAttribute('alt', 'Thumbnailed photo');
  });

  it('media_row_with_no_file_renders_a_placeholder_not_a_broken_image', async () => {
    installFetch(route(listRoutes));
    render(<MediaPage param={null} />);

    expect(await screen.findByText('Embed')).toBeInTheDocument();
    expect(screen.getAllByRole('img')).toHaveLength(2);
    expect(screen.getAllByText('Embed')).not.toHaveLength(0);
    expect(screen.getAllByRole('row')).toHaveLength(4);
    for (const image of screen.getAllByRole('img')) {
      expect(image.getAttribute('src')).not.toBe('');
      expect(image.getAttribute('src')).not.toBeNull();
    }
  });

  it('media_browse_shows_the_upstream_error_with_its_status', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Server error' } }, { status: 500 }));
    render(<MediaPage param={null} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Server error (HTTP 500)');
  });
});

describe('media detail', () => {
  const detailRoutes = {
    '/api/media/951': () => jsonResponse(loadFixture('media-951.json')),
  };

  it('media_show_renders_the_image_large_and_the_parsed_fields', async () => {
    const fetchMock = installFetch(route(detailRoutes));
    render(<MediaPage param="951" />);

    const image = await screen.findByRole('img');
    expect(image).toHaveAttribute('src', ORIGINAL);
    expect(image).toHaveAttribute('alt', 'BHC006_GMT_AF_010a.jpg');

    expect(
      screen.getByRole('heading', { name: 'BHC006_GMT_AF_010a.jpg', level: 2 }),
    ).toBeInTheDocument();
    expect(screen.getByText('Public')).toBeInTheDocument();
    expect(screen.getByText('image/jpeg')).toBeInTheDocument();
    // The renderer and ingester read as words, not as module names.
    expect(screen.getByText('The file itself')).toBeInTheDocument();
    expect(screen.getByText('Upload')).toBeInTheDocument();
    expect(screen.getByText('e82cef00da75e9c5661a9ee66155ef419cf89988.jpg')).toBeInTheDocument();
    // Once in the stage's facts line, once in the metadata table.
    expect(screen.getAllByText('5,989,397 bytes')).toHaveLength(2);
    // Both dates read the same in this capture, so both rows match.
    expect(screen.getAllByText('Oct 17, 2026, 7:55 AM')).toHaveLength(2);
    expect(screen.getByText('No values.')).toBeInTheDocument();

    expect(screen.getByRole('link', { name: '#31' })).toHaveAttribute('href', '#/items/31');
    expect(screen.getByRole('link', { name: 'Original' })).toHaveAttribute('href', ORIGINAL);

    expect(first(urlsFor(fetchMock, '/api/media/951')).pathname).toBe('/api/media/951');
  });

  it('media_show_shows_the_upstream_error_with_its_status', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Media not found' } }, { status: 404 }));
    render(<MediaPage param="951" />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Media not found (HTTP 404)');
  });
});

describe('media route', () => {
  it('media_route_rejects_a_non_numeric_id_without_fetching', () => {
    const fetchMock = installFetch(() => {
      throw new Error('unexpected fetch');
    });
    render(<MediaPage param="abc" />);

    expect(screen.getByRole('alert')).toHaveTextContent('"abc" is not a valid media id.');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe('media client', () => {
  it('a_single_media_read_uses_the_configured_base_url', async () => {
    const fetchMock = installFetch(route({ '/api/media/951': () => jsonResponse(loadFixture('media-951.json')) }));
    const mediaId = mediaIdFromParam('951');
    if (mediaId === null) {
      throw new Error('expected media id 951');
    }

    const client = new ApiClient({ baseUrl: 'http://localhost:8124' });
    const media = await client.getMedia(mediaId);

    expect(media.id).toBe(951);
    const requested = first(urlsFor(fetchMock, '/api/media/951'));
    expect(requested.origin).toBe('http://localhost:8124');
    expect(requested.href).toBe('http://localhost:8124/api/media/951');
  });
});
