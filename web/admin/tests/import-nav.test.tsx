/**
 * The Import link sits in the Content group after Media for whoever may read
 * the import routes (`GET /api/romeka/archive` answers), with or without an
 * archive root, and is hidden from whoever the route refuses.
 */
import { act, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Shell } from '../src/app/shell';
import { installFetch, jsonResponse, route, urlsFor } from './stub';

const archive = (answer: 'roots' | 'none' | 'refused') =>
  installFetch(
    route({
      '/api/romeka/archive': () =>
        answer === 'refused'
          ? jsonResponse({ errors: { error: 'Permission denied.' } }, { status: 403 })
          : jsonResponse({ roots: answer === 'roots' ? ['archive'] : [], root: answer === 'roots' ? 'archive' : null, path: '', entries: [] }),
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
  it.each(['roots', 'none'] as const)('follows Media for an admin, archive roots: %s', async (answer) => {
    window.location.hash = '#/items';
    archive(answer);
    render(<Shell />);
    expect(await screen.findByRole('link', { name: 'Import' })).toHaveAttribute('href', '#/import');
    expect(contentLinks()).toEqual(['Admin dashboard', 'Items', 'Item sets', 'Media', 'Import', 'Sites']);
  });

  it('is hidden from whoever the import routes refuse', async () => {
    window.location.hash = '#/items';
    const fetchMock = archive('refused');
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
