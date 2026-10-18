/**
 * Shell navigation test. Renders the real shell against a stubbed API and
 * checks the nav against the page table in `src/app/pages.tsx`, so the two
 * cannot drift apart.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
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
