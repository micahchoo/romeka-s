/**
 * Client auth tests: key validation at the boundary, key storage, the key on
 * every request, the sign-in form and the 403 path.
 *
 * Every request runs through the real typed client with the existing fetch
 * stub, and `localStorage` is the real jsdom store, so the assertions are about
 * what the app sends and keeps, not about a stand-in.
 */
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  KEY_CREDENTIAL_PARAM,
  KEY_IDENTITY_PARAM,
  KEY_STORAGE,
  currentKey,
  parseApiKey,
  signIn,
  signOut,
} from '../src/api/auth';
import { api } from '../src/api/client';
import { isUnknownArray, userIdFromParam } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { AuthBadge, SignInPanel } from '../src/app/auth';
import { ItemsList } from '../src/pages/items/list';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, noContent, requestUrl, route } from './stub';

const IDENTITY = 'A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6';
const CREDENTIAL = 'q7R8s9T0u1V2w3X4y5Z6a7B8c9D0e1F2';
const USER_ID = 3;

const userJson = (): unknown => {
  const page = loadFixture('users-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected the users fixture to be a page');
  }
  return page[0];
};

const mustId = <T,>(value: T | null): T => {
  if (value === null) {
    throw new Error('expected the id to parse');
  }
  return value;
};

const apiRoutes = (): Readonly<Record<string, () => Response>> => ({
  '/api': () => jsonResponse(loadFixture('api-root.json')),
  '/api/items': () => listResponse(loadFixture('items-page.json'), 2),
  [`/api/users/${String(USER_ID)}`]: () => jsonResponse(userJson()),
  '/api/users': () => jsonResponse(userJson()),
});

/** Every request the four builders send: a list, one row, a write, a delete. */
const runEveryBuilder = async (): Promise<void> => {
  await api.getApiInfo();
  await api.listItems({ search: 'cat' });
  await api.getUser(mustId(userIdFromParam(String(USER_ID))));
  await api.createUser({
    name: 'Ada',
    email: 'ada@example.org',
    role: 'global_admin',
    isActive: true,
  });
  await api.deleteUser(mustId(userIdFromParam(String(USER_ID))));
};

afterEach(() => {
  // signOut notifies mounted subscribers, so flush the update inside act.
  act(() => {
    signOut();
    setSession(null);
  });
  window.localStorage.clear();
});

describe('api key validation', () => {
  it('parse_api_key_accepts_a_pair_of_32_alphanumerics', () => {
    const parsed = parseApiKey(IDENTITY, CREDENTIAL);
    expect(parsed.ok).toBe(true);
    expect(parsed.ok ? parsed.key : null).toEqual({ identity: IDENTITY, credential: CREDENTIAL });
  });

  it('parse_api_key_rejects_a_short_identity_and_a_punctuated_credential', () => {
    const parsed = parseApiKey(IDENTITY.slice(0, 31), `${CREDENTIAL.slice(0, 31)}-`);
    expect(parsed.ok).toBe(false);
    if (parsed.ok) {
      return;
    }
    expect(parsed.errors.identity).toContain('32');
    expect(parsed.errors.credential).toContain('letters or digits');
    expect(parsed.errors.credential).not.toContain(CREDENTIAL);
  });
});

describe('api key on the wire', () => {
  it('every_api_request_carries_key_identity_and_key_credential_when_signed_in', async () => {
    signIn({ identity: IDENTITY, credential: CREDENTIAL });
    const fetchMock = installFetch(route(apiRoutes()));

    await runEveryBuilder();

    expect(fetchMock).toHaveBeenCalledTimes(5);
    for (const [input] of fetchMock.mock.calls) {
      const url = requestUrl(input);
      expect(url.searchParams.get(KEY_IDENTITY_PARAM)).toBe(IDENTITY);
      expect(url.searchParams.get(KEY_CREDENTIAL_PARAM)).toBe(CREDENTIAL);
    }
  });

  it('no_api_request_carries_a_key_parameter_when_signed_out', async () => {
    const fetchMock = installFetch(route(apiRoutes()));

    await runEveryBuilder();

    expect(fetchMock).toHaveBeenCalledTimes(5);
    for (const [input] of fetchMock.mock.calls) {
      const url = requestUrl(input);
      expect(url.searchParams.get(KEY_IDENTITY_PARAM)).toBeNull();
      expect(url.searchParams.get(KEY_CREDENTIAL_PARAM)).toBeNull();
    }
  });

  it('a_request_keeps_its_own_query_parameters_alongside_the_key', async () => {
    signIn({ identity: IDENTITY, credential: CREDENTIAL });
    const fetchMock = installFetch(route(apiRoutes()));

    await api.listItems({ search: 'cat', perPage: 10 });

    const url = requestUrl(first(fetchMock.mock.calls)[0]);
    expect(url.searchParams.get('search')).toBe('cat');
    expect(url.searchParams.get('per_page')).toBe('10');
    expect(url.searchParams.get(KEY_IDENTITY_PARAM)).toBe(IDENTITY);
  });
});

describe('api key storage', () => {
  it('sign_out_removes_the_stored_key', () => {
    signIn({ identity: IDENTITY, credential: CREDENTIAL });
    expect(window.localStorage.getItem(KEY_STORAGE)).not.toBeNull();

    signOut();

    expect(window.localStorage.getItem(KEY_STORAGE)).toBeNull();
    expect(currentKey()).toBeNull();
  });

  it('a_key_in_local_storage_is_read_back_as_a_signed_in_session', async () => {
    window.localStorage.setItem(
      KEY_STORAGE,
      JSON.stringify({ identity: IDENTITY, credential: CREDENTIAL }),
    );
    vi.resetModules();

    const reloaded = await import('../src/api/auth');

    expect(reloaded.currentKey()).toEqual({ identity: IDENTITY, credential: CREDENTIAL });
  });

  it('a_stored_value_that_is_not_a_key_pair_reads_as_signed_out', async () => {
    window.localStorage.setItem(KEY_STORAGE, JSON.stringify({ identity: IDENTITY }));
    vi.resetModules();

    const reloaded = await import('../src/api/auth');

    expect(reloaded.currentKey()).toBeNull();
  });
});

describe('sign-in form', () => {
  it('the_form_rejects_a_malformed_key_before_any_request', async () => {
    const fetchMock = installFetch(() =>
      jsonResponse({ errors: { error: 'unreachable' } }, { status: 403 }),
    );
    render(<SignInPanel />);
    fireEvent.click(screen.getByText('Use an API key'));

    fireEvent.change(screen.getByLabelText('Key identity'), {
      target: { value: IDENTITY.slice(0, 31) },
    });
    fireEvent.change(screen.getByLabelText('Key credential'), { target: { value: CREDENTIAL } });
    const form = screen.getByRole('form', { name: 'Sign in with an API key' });
    fireEvent.click(within(form).getByRole('button', { name: 'Sign in' }));

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('32');
    expect(alert).toHaveTextContent('identity');
    expect(fetchMock).not.toHaveBeenCalled();
    expect(currentKey()).toBeNull();
    expect(window.localStorage.getItem(KEY_STORAGE)).toBeNull();
  });

  it('a_valid_key_is_stored_and_the_header_reports_the_state_without_the_credential', async () => {
    installFetch(() => noContent());
    render(<AuthBadge />);
    expect(screen.getByText('Signed out')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    fireEvent.click(await screen.findByText('Use an API key'));
    const form = await screen.findByRole('form', { name: 'Sign in with an API key' });
    fireEvent.change(within(form).getByLabelText('Key identity'), { target: { value: IDENTITY } });
    fireEvent.change(within(form).getByLabelText('Key credential'), {
      target: { value: CREDENTIAL },
    });
    fireEvent.click(within(form).getByRole('button', { name: 'Sign in' }));

    expect(await screen.findByText(`Signed in as ${IDENTITY}`)).toBeInTheDocument();
    expect(screen.queryByText(CREDENTIAL)).not.toBeInTheDocument();
    expect(screen.queryByRole('form')).not.toBeInTheDocument();
    expect(currentKey()).toEqual({ identity: IDENTITY, credential: CREDENTIAL });

    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }));

    expect(await screen.findByText('Signed out')).toBeInTheDocument();
    expect(window.localStorage.getItem(KEY_STORAGE)).toBeNull();
  });
});

describe('a refused page load', () => {
  it('a_403_load_offers_the_sign_in_form_beside_the_message', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Permission denied' } }, { status: 403 }));
    render(<ItemsList />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Permission denied (HTTP 403)');
    expect(screen.getByRole('form', { name: 'Sign in with an API key' })).toBeInTheDocument();
  });

  it('a_500_load_shows_the_message_alone', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Server error' } }, { status: 500 }));
    render(<ItemsList />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Server error (HTTP 500)');
    expect(screen.queryByRole('form')).not.toBeInTheDocument();
  });
});
