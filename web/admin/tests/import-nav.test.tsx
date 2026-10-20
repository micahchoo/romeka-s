/**
 * The Import link sits in the Content group after Media while the server has
 * an archive folder (`GET /api/romeka/archive` answers `enabled: true`), and
 * is hidden when it answers `enabled: false`.
 */
import { act, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Shell } from '../src/app/shell';
import { installFetch, jsonResponse, route, urlsFor } from './stub';

const archive = (enabled: boolean) =>
  installFetch(
    route({
      '/api/romeka/archive': () => jsonResponse({ enabled, path: '', entries: [] }),
      '/api/me': () => jsonResponse({ errors: { error: 'signed out' } }, { status: 401 }),
    }),
  );

const contentLinks = (): readonly string[] => {
  const nav = screen.getByRole('navigation', { name: 'Admin pages' });
  const group = within(nav).getByRole('heading', { name: 'Content' }).closest('section');
  return group === null ? [] : within(group).getAllByRole('link').map((link) => link.textContent);
};

afterEach(() => {
  window.location.hash = '';
});

describe('Import nav link', () => {
  it('follows Media while the archive is enabled', async () => {
    window.location.hash = '#/items';
    archive(true);
    render(<Shell />);
    expect(await screen.findByRole('link', { name: 'Import' })).toHaveAttribute('href', '#/import');
    expect(contentLinks()).toEqual(['Admin dashboard', 'Items', 'Item sets', 'Media', 'Import', 'Sites']);
  });

  it('is hidden when the server has no archive folder', async () => {
    window.location.hash = '#/items';
    const fetchMock = archive(false);
    render(<Shell />);
    await screen.findByRole('navigation', { name: 'Admin pages' });
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/romeka/archive')).toHaveLength(1);
    });
    // Let the probe's answer settle before reading the nav.
    await act(async () => {});
    expect(contentLinks()).toEqual(['Admin dashboard', 'Items', 'Item sets', 'Media', 'Sites']);
  });
});
