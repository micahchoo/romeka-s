/**
 * The unsaved-changes guard on the site's Theme, Settings and Users tabs: an
 * edit marks the tab, a reload or a route change asks first, and a save
 * clears both.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const handler = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/sites/1':
      return jsonResponse(loadFixture('site-admin.json'));
    case '/api/users':
      return listResponse(loadFixture('users-page.json'), 2);
    case '/api/properties':
      return listResponse(loadFixture('properties-page.json'), 2);
    case '/api/site_pages/1':
      return jsonResponse(loadFixture('site-page-1.json'));
    default:
      return url.pathname.endsWith('/preview')
        ? new Response('<html><body>preview</body></html>', {
            headers: { 'Content-Type': 'text/html; charset=utf-8' },
          })
        : jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
  }
};

/** Whether the page asks before a reload or a closed tab. */
const unloadIsHeld = (): boolean => {
  const event = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(event);
  return event.defaultPrevented;
};

const TABS: readonly {
  readonly tab: string;
  readonly ready: () => Promise<unknown>;
  readonly edit: () => void;
}[] = [
  {
    tab: 'theme',
    ready: () => screen.findByRole('button', { name: /^Civic/ }),
    edit: () => {
      fireEvent.click(screen.getByRole('button', { name: /^Civic/ }));
    },
  },
  {
    tab: 'settings',
    ready: () => screen.findByLabelText('Name in the browser tab'),
    edit: () => {
      fireEvent.change(screen.getByLabelText('Name in the browser tab'), {
        target: { value: 'Changed' },
      });
    },
  },
  {
    tab: 'users',
    ready: () => screen.findAllByLabelText('Role'),
    edit: () => {
      const [role] = screen.getAllByLabelText('Role');
      if (role !== undefined) {
        fireEvent.change(role, { target: { value: 'viewer' } });
      }
    },
  },
];

afterEach(() => {
  vi.restoreAllMocks();
  window.location.hash = '';
});

describe('the unsaved guard on the site tabs', () => {
  for (const { tab, ready, edit } of TABS) {
    it(`${tab}: an edit marks the tab and holds a route change until the user agrees`, async () => {
      installFetch(handler);
      window.location.hash = `#/sites/1/${tab}`;
      render(<SitesPage param="1" rest={['1', tab]} />);
      await ready();
      expect(screen.queryByText('Unsaved changes')).toBeNull();
      expect(unloadIsHeld()).toBe(false);

      edit();
      expect(screen.getByText('Unsaved changes')).toBeInTheDocument();
      expect(unloadIsHeld()).toBe(true);
      const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
      window.location.hash = '#/sites/1';
      await waitFor(() => {
        expect(confirm).toHaveBeenCalledWith('This tab has unsaved changes. Leave and lose them?');
      });
      expect(window.location.hash).toBe(`#/sites/1/${tab}`);
    });

    it(`${tab}: a save clears the mark and the guard`, async () => {
      installFetch(handler);
      render(<SitesPage param="1" rest={['1', tab]} />);
      await ready();
      edit();
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));

      expect(await screen.findByText('Saved.')).toBeInTheDocument();
      expect(screen.queryByText('Unsaved changes')).toBeNull();
      expect(unloadIsHeld()).toBe(false);
    });
  }
});
