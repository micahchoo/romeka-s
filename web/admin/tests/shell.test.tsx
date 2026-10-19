/**
 * Shell navigation test. Renders the real shell against a stubbed API and
 * checks the nav against the page table in `src/app/pages.tsx`, so the two
 * cannot drift apart.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PAGES } from '../src/app/pages';
import { Shell } from '../src/app/shell';
import { installFetch, jsonResponse } from './stub';

describe('shell navigation', () => {
  it('shell_renders_one_nav_link_per_page_and_marks_the_active_route', async () => {
    window.location.hash = '#/items';
    // Every request fails, so the shell renders its nav and the error states
    // and nothing here needs a full API fixture.
    installFetch(() => jsonResponse({ errors: { error: 'offline' } }, { status: 503 }));

    render(<Shell />);
    await screen.findByRole('alert');

    const nav = screen.getByRole('navigation', { name: 'Admin pages' });
    expect(within(nav).getAllByRole('link')).toHaveLength(Object.keys(PAGES).length);

    expect(within(nav).getByRole('link', { name: 'Items' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(within(nav).getByRole('link', { name: 'Users' })).not.toHaveAttribute('aria-current');

    window.location.hash = '';
  });
});

describe('route boundary', () => {
  it('a_route_that_fails_to_load_shows_a_reload_and_keeps_the_nav', async () => {
    vi.doMock('../src/pages/modules/page', () => {
      throw new Error('chunk offline');
    });
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    try {
      window.location.hash = '#/modules';
      installFetch(() => jsonResponse({ errors: { error: 'offline' } }, { status: 503 }));
      vi.resetModules();
      const { Shell: Fresh } = await import('../src/app/shell');

      render(<Fresh />);

      expect(
        await screen.findByText('This page could not load. Check the connection, then reload.'),
      ).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Reload' })).toBeInTheDocument();
      expect(screen.getByRole('navigation', { name: 'Admin pages' })).toBeInTheDocument();
    } finally {
      quiet.mockRestore();
      vi.doUnmock('../src/pages/modules/page');
      window.location.hash = '';
    }
  });
});
