/**
 * Session tests: the email and password sign-in, the probe a reload runs, and
 * the sign-out that ends the session on the server.
 *
 * Every request runs through the real typed client against the fetch stub, and
 * `localStorage` is the real jsdom store, so the assertions are about what the
 * app sends and keeps.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { KEY_STORAGE, currentKey, signIn, signOut } from '../src/api/auth';
import { currentUser, setSession } from '../src/api/session';
import type { User } from '../src/api/types';
import { isUnknownArray, parseUser } from '../src/api/parse';
import { AuthBadge } from '../src/app/auth';
import { signInReturnPath } from '../src/app/router';
import { Shell } from '../src/app/shell';
import { loadFixture } from './loadFixture';
import {
  type Handler,
  first,
  installFetch,
  jsonResponse,
  noContent,
  requestBody,
  requestsFor,
  route,
} from './stub';

const IDENTITY = 'A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6';
const CREDENTIAL = 'q7R8s9T0u1V2w3X4y5Z6a7B8c9D0e1F2';
const EMAIL = 'ada@example.org';
const NAME = 'Ada Lovelace';

/** The first row of the user page fixture, as the wire sent it. */
const userJson = (): unknown => {
  const page = loadFixture('users-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected the users fixture to be a page');
  }
  const entry = page[0];
  if (entry === undefined) {
    throw new Error('expected a user at index 0');
  }
  return entry;
};

/** The same row parsed the way a response is, for a state the client holds. */
const fixtureUser = (): User => parseUser(userJson());

/** The shell's requests: the session probe answers, nothing else needs to. */
const shellRoutes = (me: Handler): Handler => (url, init) => {
  if (url.pathname === '/api/me') {
    return me(url, init);
  }
  return jsonResponse({ errors: { error: 'offline' } }, { status: 503 });
};

afterEach(() => {
  signOut();
  setSession(null);
  window.localStorage.clear();
  window.location.hash = '';
});

describe('password sign-in', () => {
  it('password_sign_in_posts_the_credentials_and_names_the_user', async () => {
    const fetchMock = installFetch(route({ '/api/login': () => jsonResponse(userJson()) }));
    render(<AuthBadge />);

    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    const form = await screen.findByRole('form', { name: 'Sign in' });
    fireEvent.change(within(form).getByLabelText('Email'), { target: { value: EMAIL } });
    fireEvent.change(within(form).getByLabelText('Password'), { target: { value: 'hunter2' } });
    fireEvent.click(within(form).getByRole('button', { name: 'Sign in' }));

    expect(await screen.findByText(`Signed in as ${NAME}`)).toBeInTheDocument();
    const request = first(requestsFor(fetchMock, '/api/login'));
    expect(request.init?.method).toBe('POST');
    expect(request.init?.credentials).toBe('same-origin');
    // The auth routes answer without a key, so the URL carries none.
    expect(request.url.searchParams.get('key_identity')).toBeNull();
    expect(requestBody(request)).toEqual({ email: EMAIL, password: 'hunter2' });
    expect(currentUser()?.email).toBe(EMAIL);
    // A password stays in the field until the answer, and nowhere else.
    expect(window.localStorage.length).toBe(0);
  });

  it('a_wrong_password_renders_the_servers_message_and_stays_signed_out', async () => {
    installFetch(
      route({
        '/api/login': () =>
          jsonResponse({ errors: { error: 'Invalid email or password.' } }, { status: 401 }),
      }),
    );
    render(<AuthBadge />);

    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    const form = await screen.findByRole('form', { name: 'Sign in' });
    fireEvent.change(within(form).getByLabelText('Email'), { target: { value: EMAIL } });
    fireEvent.change(within(form).getByLabelText('Password'), { target: { value: 'wrong' } });
    fireEvent.click(within(form).getByRole('button', { name: 'Sign in' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Invalid email or password. (HTTP 401)',
    );
    expect(currentUser()).toBeNull();
    expect(screen.getByText('Signed out')).toBeInTheDocument();
  });

  it('an_empty_field_is_refused_before_any_request', async () => {
    const fetchMock = installFetch(() => jsonResponse({}, { status: 500 }));
    render(<AuthBadge />);

    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    const form = await screen.findByRole('form', { name: 'Sign in' });
    fireEvent.click(within(form).getByRole('button', { name: 'Sign in' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Enter your email and password.',
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe('the session a reload keeps', () => {
  it('a_reload_reads_the_signed_in_user_from_get_api_me', async () => {
    window.location.hash = '#/items';
    const fetchMock = installFetch(shellRoutes(() => jsonResponse(userJson())));

    render(<Shell />);

    expect(await screen.findByText(`Signed in as ${NAME}`)).toBeInTheDocument();
    expect(first(requestsFor(fetchMock, '/api/me')).init?.method).toBe('GET');
    // The cookie is httpOnly, so the client stores no trace of the session.
    expect(window.localStorage.getItem(KEY_STORAGE)).toBeNull();
  });

  it('an_anonymous_probe_renders_the_sign_in_form', async () => {
    window.location.hash = '#/items';
    installFetch(
      shellRoutes(() => jsonResponse({ errors: { error: 'anonymous' } }, { status: 401 })),
    );

    render(<Shell />);

    expect(await screen.findByText('Signed out')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeInTheDocument();
    expect(currentUser()).toBeNull();
  });
});

describe('sign-out', () => {
  it('signing_out_posts_logout_and_clears_the_session_and_the_key', async () => {
    const fetchMock = installFetch(route({ '/api/logout': () => noContent() }));
    signIn({ identity: IDENTITY, credential: CREDENTIAL });
    setSession(fixtureUser());
    render(<AuthBadge />);

    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }));

    expect(await screen.findByText('Signed out')).toBeInTheDocument();
    const request = first(requestsFor(fetchMock, '/api/logout'));
    expect(request.init?.method).toBe('POST');
    expect(request.init?.credentials).toBe('same-origin');
    expect(currentUser()).toBeNull();
    expect(currentKey()).toBeNull();
    expect(window.localStorage.getItem(KEY_STORAGE)).toBeNull();
  });

  it('a_refused_logout_keeps_the_caller_signed_in_and_shows_the_message', async () => {
    installFetch(
      route({
        '/api/logout': () =>
          jsonResponse({ errors: { error: 'Server error' } }, { status: 500 }),
      }),
    );
    setSession(fixtureUser());
    render(<AuthBadge />);

    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Server error (HTTP 500)');
    expect(currentUser()?.email).toBe(EMAIL);
    expect(screen.getByText(`Signed in as ${NAME}`)).toBeInTheDocument();
  });
});

describe('the way back after a sign-in', () => {
  it('the_public_sites_log_in_link_returns_the_reader_to_its_page', () => {
    expect(signInReturnPath('?redirect=%2Fs%2Fmuseum%2Fpage%2Fwelcome')).toBe('/s/museum/page/welcome');
    expect(signInReturnPath('?redirect=%2Fs%2Fmuseum%2Fitem%2F7%3Fpage%3D2')).toBe('/s/museum/item/7?page=2');
  });

  it('arriving_from_the_public_log_in_link_opens_the_sign_in_form', () => {
    window.history.replaceState(null, '', '/login?redirect=%2Fs%2Fmuseum');
    try {
      render(<AuthBadge />);
      expect(screen.getByRole('form', { name: 'Sign in' })).toBeInTheDocument();
    } finally {
      window.history.replaceState(null, '', '/');
    }
  });

  it('a_redirect_off_this_origin_is_ignored', () => {
    for (const search of [
      '',
      '?redirect=',
      '?redirect=https%3A%2F%2Fevil.example%2F',
      '?redirect=%2F%2Fevil.example',
      '?redirect=%2F%5Cevil.example',
      '?redirect=evil.example',
      '?redirect=%2Fs%2Fa%20b',
    ]) {
      expect(signInReturnPath(search), search).toBeNull();
    }
  });
});
