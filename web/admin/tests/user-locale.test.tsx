/**
 * The admin locale a session resolves to: `MvcListeners::bootstrapLocale`'s
 * user setting, then global setting, then English.
 *
 * The server resolves the chain and answers it as `o:admin_locale` on
 * `GET /api/me`; these tests check what the SPA does with that answer, and that
 * the header switch writes the user's `locale` setting back.
 */
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isUnknownArray, isRecord } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { Shell } from '../src/app/shell';
import { LOCALE_STORAGE, setLocale } from '../src/i18n';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';
import type { Handler } from './stub';

/** The first user of the users fixture, with the members `GET /api/me` adds. */
const meJson = (extra: Record<string, unknown> = {}): Record<string, unknown> => {
  const page = loadFixture('users-page.json');
  const entry = isUnknownArray(page) ? page[0] : undefined;
  if (!isRecord(entry)) {
    throw new Error('expected a user at index 0');
  }
  return { ...entry, ...extra };
};

const userPath = (): string => `/api/users/${String(meJson()['o:id'])}`;

/** `/api/me` answers the user; a user write answers `write`; anything else is offline. */
const sessionRoutes =
  (me: Record<string, unknown>, write: () => Response = () => jsonResponse(me)): Handler =>
  (url) => {
    if (url.pathname === '/api/me') {
      return jsonResponse(me);
    }
    if (url.pathname === userPath()) {
      return write();
    }
    return jsonResponse({ errors: { error: 'offline' } }, { status: 503 });
  };

afterEach(async () => {
  setSession(null);
  await act(() => setLocale('en'));
  window.localStorage.clear();
  window.location.hash = '';
});

describe('the resolved admin locale', () => {
  it('a_signed_in_users_resolved_locale_replaces_a_different_stored_one', async () => {
    window.localStorage.setItem(LOCALE_STORAGE, 'en');
    installFetch(sessionRoutes(meJson({ 'o:admin_locale': 'es' })));

    render(<Shell />);

    expect(await screen.findByText('Administración de Romeka S')).toBeInTheDocument();
    expect(document.documentElement.lang).toBe('es');
    // The next reload paints the server's choice first.
    expect(window.localStorage.getItem(LOCALE_STORAGE)).toBe('es');
  });

  it('an_upstream_region_tag_narrows_and_an_unknown_one_falls_back_to_english', async () => {
    installFetch(sessionRoutes(meJson({ 'o:admin_locale': 'es_ES' })));
    const { unmount } = render(<Shell />);
    expect(await screen.findByText('Administración de Romeka S')).toBeInTheDocument();
    unmount();

    installFetch(sessionRoutes(meJson({ 'o:admin_locale': 'kl_GL' })));
    render(<Shell />);
    expect(await screen.findByText('Romeka S admin')).toBeInTheDocument();
    await vi.waitFor(() => {
      expect(window.localStorage.getItem(LOCALE_STORAGE)).toBe('en');
    });
  });

  it('with_no_locale_setting_on_the_server_the_stored_choice_stays', async () => {
    window.localStorage.setItem(LOCALE_STORAGE, 'es');
    await act(() => setLocale('es'));
    const fetchMock = installFetch(sessionRoutes(meJson()));

    render(<Shell />);
    await vi.waitFor(() => {
      expect(requestsFor(fetchMock, '/api/me')).toHaveLength(1);
    });

    expect(screen.getByText('Administración de Romeka S')).toBeInTheDocument();
  });
});

describe('the header switch', () => {
  it('changing_the_switch_writes_the_user_setting', async () => {
    const fetchMock = installFetch(sessionRoutes(meJson()));
    render(<Shell />);
    await screen.findByText(/Signed in as/u);

    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'es' } });

    expect(await screen.findByText('Administración de Romeka S')).toBeInTheDocument();
    await vi.waitFor(() => {
      expect(requestsFor(fetchMock, userPath())).toHaveLength(1);
    });
    const write = first(requestsFor(fetchMock, userPath()));
    expect(write.init?.method).toBe('PATCH');
    expect(requestBody(write)).toEqual({ 'o:user_settings': { locale: 'es' } });
  });

  it('a_refused_write_leaves_the_switch_working_and_shows_no_error', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const fetchMock = installFetch(
      sessionRoutes(meJson(), () => jsonResponse({ errors: { error: 'down' } }, { status: 500 })),
    );
    render(<Shell />);
    await screen.findByText(/Signed in as/u);

    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'es' } });

    expect(await screen.findByText('Administración de Romeka S')).toBeInTheDocument();
    await vi.waitFor(() => {
      expect(consoleError).toHaveBeenCalled();
    });
    expect(requestsFor(fetchMock, userPath())).toHaveLength(1);
    // The dashboard behind the header renders its own offline alert; none is the write's.
    expect(screen.queryByText(/down/u)).toBeNull();
    expect(window.localStorage.getItem(LOCALE_STORAGE)).toBe('es');
    consoleError.mockRestore();
  });

  it('a_signed_out_switch_writes_nothing_to_the_server', async () => {
    const fetchMock = installFetch(() =>
      jsonResponse({ errors: { error: 'Authentication required.' } }, { status: 401 }),
    );
    render(<Shell />);
    await vi.waitFor(() => {
      expect(requestsFor(fetchMock, '/api/me')).toHaveLength(1);
    });

    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'es' } });

    expect(await screen.findByText('Administración de Romeka S')).toBeInTheDocument();
    expect(fetchMock.mock.calls.some(([, init]) => init?.method === 'PATCH')).toBe(false);
  });
});
