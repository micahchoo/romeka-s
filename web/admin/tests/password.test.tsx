/**
 * Set-password and forgot tests, through the real client and the real pages.
 *
 * The set-password key comes from the URL, so the route parser is tested here
 * too: it is the only place the shapes a mail link can take are read.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ForgotPasswordPage } from '../src/app/forgot';
import { PasswordPage } from '../src/app/password';
import { parseAuthRoute } from '../src/app/router';
import {
  first,
  installFetch,
  jsonResponse,
  noContent,
  requestBody,
  requestsFor,
  route,
} from './stub';

const KEY = 'K3y'.repeat(10);

describe('the auth URL', () => {
  it('parse_auth_route_reads_the_key_from_every_shape_a_link_can_take', () => {
    expect(parseAuthRoute(`#/password/${KEY}`, '')).toEqual({ kind: 'password', key: KEY });
    expect(parseAuthRoute(`#/password?key=${KEY}`, '')).toEqual({ kind: 'password', key: KEY });
    expect(parseAuthRoute(`#/create-password?key=${KEY}`, '')).toEqual({
      kind: 'password',
      key: KEY,
    });
    expect(parseAuthRoute('#/items', `?key=${KEY}`)).toEqual({ kind: 'password', key: KEY });
  });

  it('parse_auth_route_separates_forgot_from_an_admin_route', () => {
    expect(parseAuthRoute('#/forgot', '')).toEqual({ kind: 'forgot' });
    expect(parseAuthRoute('#/items', '')).toBeNull();
    // A keyless password link is still the set-password page, which says so.
    expect(parseAuthRoute('#/password', '')).toEqual({ kind: 'password', key: null });
  });
});

describe('set a password', () => {
  it('set_password_posts_the_url_key_and_the_new_password', async () => {
    const fetchMock = installFetch(route({ '/api/password': () => jsonResponse({}) }));
    render(<PasswordPage passwordKey={KEY} />);

    fireEvent.change(screen.getByLabelText('New password'), { target: { value: 'hunter2' } });
    fireEvent.change(screen.getByLabelText('Confirm the new password'), {
      target: { value: 'hunter2' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Set the password' }));

    expect(
      await screen.findByText('Your password is set. Sign in from the header to continue.'),
    ).toBeInTheDocument();
    const request = first(requestsFor(fetchMock, '/api/password'));
    expect(request.init?.method).toBe('POST');
    expect(request.init?.credentials).toBe('same-origin');
    expect(requestBody(request)).toEqual({ key: KEY, password: 'hunter2' });
  });

  it('an_expired_key_renders_the_servers_own_message', async () => {
    installFetch(
      route({
        '/api/password': () =>
          jsonResponse(
            { errors: { error: 'The password key is invalid or expired.' } },
            { status: 404 },
          ),
      }),
    );
    render(<PasswordPage passwordKey={KEY} />);

    fireEvent.change(screen.getByLabelText('New password'), { target: { value: 'hunter2' } });
    fireEvent.change(screen.getByLabelText('Confirm the new password'), {
      target: { value: 'hunter2' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Set the password' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The password key is invalid or expired. (HTTP 404)',
    );
  });

  it('a_mismatched_confirmation_is_refused_before_any_request', async () => {
    const fetchMock = installFetch(() => jsonResponse({}, { status: 500 }));
    render(<PasswordPage passwordKey={KEY} />);

    fireEvent.change(screen.getByLabelText('New password'), { target: { value: 'hunter2' } });
    fireEvent.change(screen.getByLabelText('Confirm the new password'), {
      target: { value: 'hunter3' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Set the password' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The two passwords do not match.',
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('a_link_with_no_key_says_so_and_offers_the_request_form', () => {
    render(<PasswordPage passwordKey={null} />);

    expect(screen.getByRole('alert')).toHaveTextContent(
      'This link carries no password key. Request a new one.',
    );
    expect(screen.getByRole('link', { name: 'Reset your password' })).toHaveAttribute(
      'href',
      '#/forgot',
    );
  });
});

describe('forgot a password', () => {
  it('forgot_posts_the_address_and_answers_without_saying_whether_it_exists', async () => {
    const fetchMock = installFetch(route({ '/api/password/forgot': () => noContent() }));
    render(<ForgotPasswordPage />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'ada@example.org' } });
    fireEvent.click(screen.getByRole('button', { name: 'Send the reset link' }));

    expect(await screen.findByText('Check your mail for the reset link.')).toBeInTheDocument();
    const request = first(requestsFor(fetchMock, '/api/password/forgot'));
    expect(request.init?.method).toBe('POST');
    expect(request.init?.credentials).toBe('same-origin');
    expect(requestBody(request)).toEqual({ email: 'ada@example.org' });
  });

  it('forgot_refuses_an_empty_address_before_any_request', async () => {
    const fetchMock = installFetch(() => noContent());
    render(<ForgotPasswordPage />);

    fireEvent.click(screen.getByRole('button', { name: 'Send the reset link' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Enter your email address.');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
