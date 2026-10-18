/**
 * User write tests: the invite form and the edit form through the real client.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isUnknownArray } from '../src/api/parse';
import { UsersPage } from '../src/pages/users/page';
import { loadFixture } from './loadFixture';
import {
  first,
  installFetch,
  jsonResponse,
  requestBody,
  requestsFor,
} from './stub';

const userFixture = (index: number): unknown => {
  const page = loadFixture('users-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected a user list fixture');
  }
  const entry = page[index];
  if (entry === undefined) {
    throw new Error(`expected a user fixture at index ${String(index)}`);
  }
  return entry;
};

const KEY = 'K3y'.repeat(10);
const LINK = `http://localhost/#/password/${KEY}`;

/** The answer to a mint, with mail either configured or not. */
const linkAnswer = (sent: boolean): unknown => ({ key: KEY, link: LINK, sent });

/** jsdom ships no clipboard, so a test installs the one it needs. */
const stubClipboard = (writeText: () => Promise<void>): void => {
  Object.defineProperty(window.navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
  });
};

afterEach(() => {
  Reflect.deleteProperty(window.navigator, 'clipboard');
});

describe('user invite', () => {
  it('an_invite_creates_the_user_and_then_mints_the_password_link', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/users' && init?.method === 'POST') {
        return jsonResponse(userFixture(0));
      }
      if (url.pathname === '/api/users/3/password-link' && init?.method === 'POST') {
        return jsonResponse(linkAnswer(false));
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<UsersPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Display name'), {
      target: { value: 'New User' },
    });
    fireEvent.change(screen.getByLabelText('Email'), {
      target: { value: 'new@example.com' },
    });
    fireEvent.change(screen.getByLabelText('Role'), { target: { value: 'author' } });
    fireEvent.click(screen.getByRole('button', { name: 'Invite this user' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/users').filter(
          (request) => request.init?.method === 'POST',
        ),
      ).toHaveLength(1);
    });
    const post = requestsFor(fetchMock, '/api/users').find(
      (request) => request.init?.method === 'POST',
    );
    if (post === undefined) {
      throw new Error('expected a POST to /api/users');
    }
    expect(requestBody(post)).toEqual({
      'o:name': 'New User',
      'o:email': 'new@example.com',
      'o:role': 'author',
      'o:is_active': false,
    });

    // The create writes no password, so the link for the new row is the invite.
    const mint = first(requestsFor(fetchMock, '/api/users/3/password-link'));
    expect(mint.init?.method).toBe('POST');
    expect(await screen.findByLabelText('Password link for New User')).toHaveValue(LINK);
    expect(screen.getByText('No mail was sent, so hand this link to the user.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Edit this user' })).toHaveAttribute(
      'href',
      '#/users/3',
    );
  });

  it('an_invite_names_the_address_the_mail_went_to', async () => {
    installFetch((url, init) => {
      if (url.pathname === '/api/users' && init?.method === 'POST') {
        return jsonResponse(userFixture(0));
      }
      if (url.pathname === '/api/users/3/password-link' && init?.method === 'POST') {
        return jsonResponse(linkAnswer(true));
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<UsersPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Display name'), {
      target: { value: 'New User' },
    });
    fireEvent.change(screen.getByLabelText('Email'), {
      target: { value: 'new@example.com' },
    });
    fireEvent.change(screen.getByLabelText('Role'), { target: { value: 'author' } });
    fireEvent.click(screen.getByRole('button', { name: 'Invite this user' }));

    expect(
      await screen.findByText('The invitation mail went to new@example.com.'),
    ).toBeInTheDocument();
  });

  it('the_copy_control_puts_the_link_on_the_clipboard', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    stubClipboard(writeText);
    installFetch((url, init) => {
      if (url.pathname === '/api/users/3' && init?.method === 'PUT') {
        return jsonResponse(userFixture(0));
      }
      if (url.pathname === '/api/users/3/password-link' && init?.method === 'POST') {
        return jsonResponse(linkAnswer(false));
      }
      if (url.pathname === '/api/users/3') {
        return jsonResponse(userFixture(0));
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<UsersPage param="3" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Create a password link' }));

    fireEvent.click(await screen.findByRole('button', { name: 'Copy link' }));

    expect(await screen.findByText('Link copied.')).toBeInTheDocument();
    expect(writeText).toHaveBeenCalledWith(LINK);
  });

  it('a_refused_clipboard_says_to_copy_the_link_by_hand', async () => {
    stubClipboard(() => Promise.reject(new Error('denied')));
    installFetch((url, init) => {
      if (url.pathname === '/api/users/3/password-link' && init?.method === 'POST') {
        return jsonResponse(linkAnswer(false));
      }
      if (url.pathname === '/api/users/3') {
        return jsonResponse(userFixture(0));
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<UsersPage param="3" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Create a password link' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Copy link' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The browser refused clipboard access. Select the link and copy it by hand.',
    );
  });
});

describe('user edit form', () => {
  it('user_edit_renders_the_stored_row_and_puts_the_changed_fields', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/users/3' && init?.method === 'PUT') {
        return jsonResponse(userFixture(0));
      }
      if (url.pathname === '/api/users/3') {
        return jsonResponse(userFixture(0));
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<UsersPage param="3" />);

    expect(await screen.findByLabelText('Display name')).toHaveValue('Ada Lovelace');
    expect(screen.getByLabelText('Email')).toHaveValue('ada@example.org');
    expect(screen.getByLabelText('Role')).toHaveValue('global_admin');
    expect(screen.getByLabelText('Active')).toBeChecked();

    fireEvent.change(screen.getByLabelText('Display name'), {
      target: { value: 'Ada Byron' },
    });
    fireEvent.change(screen.getByLabelText('Role'), { target: { value: 'reviewer' } });
    fireEvent.click(screen.getByLabelText('Active'));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const put = requestsFor(fetchMock, '/api/users/3').find(
      (request) => request.init?.method === 'PUT',
    );
    if (put === undefined) {
      throw new Error('expected a PUT to /api/users/3');
    }
    expect(put.init?.method).toBe('PUT');
    expect(requestBody(put)).toEqual({
      'o:name': 'Ada Byron',
      'o:email': 'ada@example.org',
      'o:role': 'reviewer',
      'o:is_active': false,
    });
  });

  it('user_edit_surfaces_a_403_permission_message', async () => {
    installFetch((url, init) => {
      if (url.pathname === '/api/users/3' && init?.method === 'PUT') {
        return jsonResponse(
          {
            errors: {
              error:
                'Permission denied for the current user to update the Omeka\\Entity\\User resource.',
            },
          },
          { status: 403 },
        );
      }
      if (url.pathname === '/api/users/3') {
        return jsonResponse(userFixture(0));
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<UsersPage param="3" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Permission denied for the current user to update the Omeka\\Entity\\User resource. (HTTP 403)',
    );
  });
});
