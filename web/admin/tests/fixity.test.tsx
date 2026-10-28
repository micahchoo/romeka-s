/**
 * The fixity check on its screens: the Jobs screen's start control, a check's result on its job
 * page, the dashboard line, and the Media list a count opens. Every count is the server's; the
 * screens only link each to the list whose total it is.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { User } from '../src/api/types';
import { FixityLine } from '../src/pages/dashboard/fixity-line';
import { JobsPage } from '../src/pages/jobs/page';
import { MediaList } from '../src/pages/media/list';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestsFor, route, urlsFor } from './stub';

const user = (role: string): User => ({
  id: brandId<'UserId'>(1),
  apiUrl: 'http://localhost/api/users/1',
  types: ['o:User'],
  name: 'Reader',
  email: 'reader@example.com',
  created: '2026-10-16T09:00:00+00:00',
  role,
  isActive: true,
  localeSetting: null,
  lessons: { hidden: false, seen: [] },
  savedLists: [],
  adminLocale: null,
});

const checkJob = (ended: string | null) => ({
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/jobs/12',
  '@type': ['o:Job'],
  'o:id': 12,
  'o:status': ended === null ? 'in_progress' : 'completed',
  'o:job_class': 'Romeka\\Job\\FixityCheck',
  'o:args': {},
  'o:owner': null,
  'o:started': { '@value': '2026-10-28T02:00:00+00:00', '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
  'o:ended':
    ended === null ? null : { '@value': ended, '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
});

const check = {
  job: 12,
  status: 'completed',
  started: '2026-10-28T02:00:00+00:00',
  ended: '2026-10-28T02:20:00+00:00',
  checked: 4230,
  ok: 4227,
  differs: 2,
  missing: 1,
  unreadable: 0,
};

afterEach(() => {
  setSession(null);
  window.location.hash = '';
});

describe('a check on its job page', () => {
  it('states the counts, each a link to the Media list of those files', async () => {
    installFetch(
      route({
        '/api/jobs/12': () => jsonResponse(checkJob(check.ended)),
        '/api/romeka/jobs/12/log': () => new Response('Checked 4230 files.\n'),
        '/api/romeka/fixity/12': () => jsonResponse(check),
      }),
    );
    render(<JobsPage param="12" />);

    const checked = await screen.findByRole('link', { name: /^4,230 files checked on .+\.$/ });
    expect(checked).toHaveAttribute('href', '#/media?fixity=checked&fixityJob=12');
    expect(screen.getByRole('link', { name: '2 differ.' })).toHaveAttribute(
      'href',
      '#/media?fixity=differs&fixityJob=12',
    );
    expect(screen.getByRole('link', { name: '1 missing.' })).toHaveAttribute(
      'href',
      '#/media?fixity=missing&fixityJob=12',
    );
    expect(screen.queryByRole('link', { name: /cannot be read/ })).not.toBeInTheDocument();
    expect(screen.getAllByText('Check stored files').length).toBeGreaterThan(0);
  });

  it('waits for a running check, and reads no result until it ends', async () => {
    const fetchMock = installFetch(
      route({
        '/api/jobs/12': () => jsonResponse(checkJob(null)),
        '/api/romeka/jobs/12/log': () => new Response(''),
      }),
    );
    render(<JobsPage param="12" />);

    expect(await screen.findByText('The result shows here when the check ends.')).toBeInTheDocument();
    expect(urlsFor(fetchMock, '/api/romeka/fixity/12')).toHaveLength(0);
  });

  it('says so when the check ended without a result', async () => {
    installFetch(
      route({
        '/api/jobs/12': () => jsonResponse({ ...checkJob(check.ended), 'o:status': 'error' }),
        '/api/romeka/jobs/12/log': () => new Response('database is locked\n'),
        '/api/romeka/fixity/12': () => jsonResponse({ errors: { error: 'Not found' } }, { status: 404 }),
      }),
    );
    render(<JobsPage param="12" />);

    expect(
      await screen.findByText('This check ended before it wrote a result. Its log says why.'),
    ).toBeInTheDocument();
  });
});

describe('the start control', () => {
  const jobs = {
    '/api/jobs': () => jsonResponse(loadFixture('jobs-page.json')),
    '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  };

  it('starts a check for an administrator and opens its job', async () => {
    setSession(user('site_admin'));
    const fetchMock = installFetch(
      route({ ...jobs, '/api/romeka/fixity': () => jsonResponse({ job: 7 }, { status: 201 }) }),
    );
    render(<JobsPage />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check stored files' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/jobs/7');
    });
    expect(requestsFor(fetchMock, '/api/romeka/fixity').map((sent) => sent.init?.method)).toEqual(['POST']);
  });

  it('is not offered to an editor, who may read jobs but not write one', async () => {
    setSession(user('editor'));
    installFetch(route(jobs));
    render(<JobsPage />);

    expect(await screen.findByRole('link', { name: '1' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Check stored files' })).not.toBeInTheDocument();
  });
});

describe('the dashboard line', () => {
  it('names the problems of the latest check and opens it', async () => {
    setSession(user('editor'));
    installFetch(route({ '/api/romeka/fixity': () => jsonResponse({ check, running: null }) }));
    render(<FixityLine />);

    const line = await screen.findByRole('link', { name: /^Files last checked .+: 3 problems$/ });
    expect(line).toHaveAttribute('href', '#/jobs/12');
  });

  it('says when no check has run', async () => {
    setSession(user('global_admin'));
    installFetch(route({ '/api/romeka/fixity': () => jsonResponse({ check: null, running: null }) }));
    render(<FixityLine />);

    expect(await screen.findByRole('link', { name: 'Stored files have never been checked.' })).toHaveAttribute(
      'href',
      '#/jobs',
    );
  });

  it('asks nothing for an author, who may not read jobs', () => {
    setSession(user('author'));
    const fetchMock = installFetch(route({}));
    const { container } = render(<FixityLine />);

    expect(container).toBeEmptyDOMElement();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe('the Media list a count opens', () => {
  it('asks the server for those files, says which they are, and clears back to all media', async () => {
    window.location.hash = '#/media?fixity=differs&fixityJob=12';
    const fetchMock = installFetch(
      route({ '/api/media': () => listResponse(loadFixture('media-page.json'), 2) }),
    );
    render(<MediaList />);

    expect(await screen.findByText('Files that differ in check #12.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open the check' })).toHaveAttribute('href', '#/jobs/12');
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/media').length).toBeGreaterThan(0);
    });
    const asked = urlsFor(fetchMock, '/api/media')[0];
    expect(asked?.searchParams.get('romeka_fixity')).toBe('differs');
    expect(asked?.searchParams.get('romeka_fixity_job')).toBe('12');

    fireEvent.click(screen.getByRole('button', { name: 'Show all media' }));

    expect(window.location.hash).toBe('#/media');
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/media').at(-1)?.searchParams.has('romeka_fixity')).toBe(false);
    });
    expect(screen.queryByText('Files that differ in check #12.')).not.toBeInTheDocument();
  });
});
