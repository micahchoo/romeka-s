/**
 * Shell navigation test. Renders the real shell against a stubbed API and
 * checks the nav against the page table in `src/app/pages.tsx`, so the two
 * cannot drift apart.
 */
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { signIn, signOut } from '../src/api/auth';
import { setSession } from '../src/api/session';
import { PAGES } from '../src/app/pages';
import { Shell } from '../src/app/shell';
import { installFetch, jsonResponse, meJson, requestUrl, signedIn, urlsFor } from './stub';

const offline = (): Response => jsonResponse({ errors: { error: 'offline' } }, { status: 503 });

afterEach(() => {
  signOut();
  setSession(null);
  window.localStorage.clear();
  window.location.hash = '';
});

const navLinks = (): readonly (string | null)[] =>
  within(screen.getByRole('navigation', { name: 'Admin pages' }))
    .getAllByRole('link')
    .map((link) => link.textContent);

describe('shell navigation', () => {
  it('shell_renders_one_nav_link_per_page_and_marks_the_active_route', async () => {
    window.location.hash = '#/items';
    // Every request but the session probe fails, so the shell renders its nav
    // and the error states and nothing here needs a full API fixture.
    installFetch(signedIn(offline));

    render(<Shell />);
    await screen.findByRole('alert');

    const nav = screen.getByRole('navigation', { name: 'Admin pages' });
    // A global admin is offered every page.
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
    vi.doMock('../src/pages/system/page', () => {
      throw new Error('chunk offline');
    });
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    try {
      window.location.hash = '#/system';
      installFetch(signedIn(offline));
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
      vi.doUnmock('../src/pages/system/page');
      window.location.hash = '';
    }
  });
});

describe('signed out', () => {
  it.each(['', '#/items', '#/users/3', '#/settings'])(
    'the address %s shows only the sign-in panel and asks the API nothing else',
    async (hash) => {
      window.location.hash = hash;
      const fetchMock = installFetch((url) =>
        url.pathname === '/api/me'
          ? jsonResponse({ errors: { error: 'anonymous' } }, { status: 401 })
          : offline(),
      );

      render(<Shell />);

      expect(await screen.findByRole('textbox', { name: 'Email' })).toBeInTheDocument();
      expect(screen.getByText('Sign in to manage this collection.')).toBeInTheDocument();
      expect(screen.queryByRole('navigation', { name: 'Admin pages' })).not.toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      const asked = fetchMock.mock.calls.map(([input]) => requestUrl(input).pathname);
      expect(asked).toEqual(['/api/me']);
    },
  );

  it('shows neither the panel nor the pages until the session probe answers', async () => {
    window.location.hash = '#/items';
    let answer: (response: Response) => void = () => undefined;
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>((input) =>
        requestUrl(input).pathname === '/api/me'
          ? new Promise<Response>((resolve) => {
              answer = resolve;
            })
          : Promise.resolve(offline()),
      ),
    );
    // A fresh session module: the page has just loaded and nobody has asked yet.
    vi.resetModules();
    const { Shell: Fresh } = await import('../src/app/shell');

    render(<Fresh />);

    expect(await screen.findByRole('status')).toHaveTextContent('Loading page...');
    expect(screen.queryByRole('textbox', { name: 'Email' })).not.toBeInTheDocument();
    expect(screen.queryByRole('navigation', { name: 'Admin pages' })).not.toBeInTheDocument();
    act(() => {
      answer(jsonResponse(meJson()));
    });
    expect(await screen.findByRole('navigation', { name: 'Admin pages' })).toBeInTheDocument();
  });

  it('signing in opens the page the address names', async () => {
    window.location.hash = '#/users';
    installFetch((url) => {
      if (url.pathname === '/api/me') {
        return jsonResponse({ errors: { error: 'anonymous' } }, { status: 401 });
      }
      if (url.pathname === '/api/login') {
        return jsonResponse(meJson());
      }
      return offline();
    });

    render(<Shell />);
    fireEvent.change(await screen.findByRole('textbox', { name: 'Email' }), {
      target: { value: 'ada@example.org' },
    });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'secret' } });
    fireEvent.click(
      within(screen.getByRole('form', { name: 'Sign in' })).getByRole('button', { name: 'Sign in' }),
    );

    expect(await screen.findByRole('heading', { level: 1, name: 'Users' })).toBeInTheDocument();
    expect(window.location.hash).toBe('#/users');
    expect(
      within(screen.getByRole('navigation', { name: 'Admin pages' })).getByRole('link', { name: 'Users' }),
    ).toHaveAttribute('aria-current', 'page');
  });
});

describe('the nav by role', () => {
  const ADMIN_ONLY = ['Import', 'Settings', 'Modules', 'Jobs'];

  it.each([
    ['global_admin', []],
    ['site_admin', ['Settings']],
    ['editor', ['Import', 'Settings']],
    ['reviewer', ['Import', 'Settings']],
    ['author', ADMIN_ONLY],
    ['researcher', ADMIN_ONLY],
  ])('a %s is not offered %j', async (role, hidden) => {
    window.location.hash = '#/items';
    installFetch(signedIn(offline, role));

    render(<Shell />);
    await screen.findByRole('alert');

    const links = navLinks();
    for (const name of ADMIN_ONLY) {
      expect(links.includes(name), name).toBe(!hidden.includes(name));
    }
    expect(links).toContain('Users');
    expect(links).toContain('Items');
  });

  it('a caller signed in by an API key alone is offered only the pages every role may use', async () => {
    window.location.hash = '#/items';
    signIn({ identity: 'A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6', credential: 'q7R8s9T0u1V2w3X4y5Z6a7B8c9D0e1F2' });
    const fetchMock = installFetch((url) =>
      url.pathname === '/api/me'
        ? jsonResponse({ errors: { error: 'anonymous' } }, { status: 401 })
        : offline(),
    );

    render(<Shell />);
    await screen.findByRole('alert');
    expect(urlsFor(fetchMock, '/api/me')).toHaveLength(1);

    const links = navLinks();
    for (const name of ADMIN_ONLY) {
      expect(links, name).not.toContain(name);
    }
    expect(links).toContain('Items');
  });
});
