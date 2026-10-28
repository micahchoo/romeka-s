/**
 * The dashboard's "Needs attention" list: each count is the server's, each non-zero count opens
 * the Items list it counts, a zero stays a plain line, and nothing shows a number before the
 * server answers.
 */
import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setSession } from '../src/api/session';
import { NeedsAttention } from '../src/pages/dashboard/needs-attention';
import { installFetch, jsonResponse, route } from './stub';

const counts = (noItemSet: number, noItemType: number, changed: number) => ({
  no_item_set: noItemSet,
  no_resource_template: noItemType,
  changed_by_others: { since: '2026-10-20T08:30:00Z', items: changed },
});

afterEach(() => {
  setSession(null);
});

describe('needs attention', () => {
  it('links each count to the Items list it counts', async () => {
    installFetch(route({ '/api/romeka/dashboard': () => jsonResponse(counts(3, 1240, 2)) }));
    render(<NeedsAttention />);

    expect(await screen.findByRole('link', { name: 'Items in no item set: 3' })).toHaveAttribute(
      'href',
      '#/items?set=none',
    );
    expect(screen.getByRole('link', { name: 'Items with no item type: 1,240' })).toHaveAttribute(
      'href',
      '#/items?template=none',
    );
    expect(screen.getByRole('link', { name: 'Items others changed in the last 7 days: 2' })).toHaveAttribute(
      'href',
      '#/items?changed=2026-10-20T08%3A30%3A00Z',
    );
  });

  it('states a zero as a plain line, not a link to an empty list', async () => {
    installFetch(route({ '/api/romeka/dashboard': () => jsonResponse(counts(0, 4, 0)) }));
    render(<NeedsAttention />);

    expect(await screen.findByText('Items in no item set: 0')).not.toHaveAttribute('href');
    expect(screen.queryByRole('link', { name: /no item set/ })).toBeNull();
    expect(screen.getByRole('link', { name: 'Items with no item type: 4' })).toBeInTheDocument();
  });

  it('says it is counting, with no number, until the server answers', () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(() => new Promise<Response>(() => undefined)));
    render(<NeedsAttention />);

    expect(screen.getByRole('status')).toHaveTextContent('Counting the items that need attention...');
    expect(screen.queryByText(/: 0$/)).toBeNull();
  });
});
