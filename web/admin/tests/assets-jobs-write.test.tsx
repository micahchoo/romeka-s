/**
 * Write tests for assets and jobs. Both are list-and-detail resources the API
 * serves `PUT`/`PATCH` and `DELETE` for, with no create: an asset create needs
 * a multipart file and a job create needs a `class` the dispatcher sets. The
 * tests run each form through the real client and parsers against a stubbed
 * fetch.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Mock } from 'vitest';
import { AssetsPage } from '../src/pages/assets/page';
import { JobsPage } from '../src/pages/jobs/page';
import { loadFixture } from './loadFixture';
import {
  detached,
  first,
  installFetch,
  jsonResponse,
  requestBody,
  requestsFor,
  type CapturedRequest,
  type Handler,
} from './stub';

afterEach(() => {
  window.location.hash = '';
});

const asset = (): unknown => {
  const page = loadFixture('assets-page.json');
  if (!Array.isArray(page)) {
    throw new Error('expected an asset list fixture');
  }
  return page[0];
};

const job = (): unknown => {
  const page = loadFixture('jobs-page.json');
  if (!Array.isArray(page)) {
    throw new Error('expected a job list fixture');
  }
  return page[0];
};

const jobNoStatus = (): unknown => {
  const page = loadFixture('job-no-status.json');
  if (!Array.isArray(page)) {
    throw new Error('expected a job list fixture');
  }
  return page[0];
};

const notFound = (): Response =>
  jsonResponse({ errors: { error: 'Unexpected request' } }, { status: 404 });

const withFixture =
  (routes: Readonly<Record<string, Handler>>): Handler =>
  (url, init) => {
    const handler = routes[url.pathname];
    return handler === undefined ? notFound() : handler(url, init);
  };

const sent = (
  fetchMock: Mock<typeof fetch>,
  pathname: string,
  method: string,
): CapturedRequest =>
  first(requestsFor(fetchMock, pathname).filter((request) => request.init?.method === method));

describe('the asset detail view', () => {
  it('asset_list_links_a_row_to_its_detail_view', async () => {
    installFetch(
      withFixture({
        '/api/assets': () => jsonResponse(loadFixture('assets-page.json')),
        '/api/users': () => jsonResponse(loadFixture('users-page.json')),
      }),
    );
    render(<AssetsPage />);

    expect(await screen.findByRole('link', { name: 'omeka-logo.png' })).toHaveAttribute(
      'href',
      '#/assets/1',
    );
  });

  it('asset_edit_puts_the_name_and_the_alt_text', async () => {
    const fetchMock = installFetch(
      withFixture({
        '/api/assets/1': () => jsonResponse(asset()),
      }),
    );
    render(<AssetsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this asset' }));
    fireEvent.change(await screen.findByLabelText('Name'), {
      target: { value: 'omeka-logo-revised.png' },
    });
    fireEvent.change(screen.getByLabelText('Alt text'), { target: { value: 'The logo' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const put = await waitFor(() => sent(fetchMock, '/api/assets/1', 'PUT'));
    const body = requestBody(put);
    expect(put.init?.method).toBe('PUT');
    expect(body).toEqual({ 'o:name': 'omeka-logo-revised.png', 'o:alt_text': 'The logo' });
  });

  it('asset_edit_refuses_an_empty_name_before_any_request', async () => {
    const fetchMock = installFetch(
      withFixture({
        '/api/assets/1': () => jsonResponse(asset()),
      }),
    );
    render(<AssetsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this asset' }));
    fireEvent.change(await screen.findByLabelText('Name'), { target: { value: '  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('An asset must have a name.');
    expect(requestsFor(fetchMock, '/api/assets/1')).toHaveLength(1);
  });

  it('asset_edit_renders_the_api_validation_message', async () => {
    installFetch((url, init) => {
      if (url.pathname === '/api/assets/1' && init?.method === 'PUT') {
        return jsonResponse(
          { errors: { 'o:name': ['An asset must have a name.'] } },
          { status: 422 },
        );
      }
      return withFixture({
        '/api/assets/1': () => jsonResponse(asset()),
      })(url, init);
    });
    render(<AssetsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this asset' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'An asset must have a name. (HTTP 422)',
    );
  });

  it('asset_delete_sends_delete_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(
      withFixture({
        '/api/assets/1': (_url, init) =>
          jsonResponse(init?.method === 'DELETE' ? detached(asset()) : asset()),
      }),
    );
    render(<AssetsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this asset' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete omeka-logo.png' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/assets/1').filter(
          (request) => request.init?.method === 'DELETE',
        ),
      ).toHaveLength(1);
    });
    expect(window.location.hash).toBe('#/assets');
  });

  it('asset_detail_reports_an_unparseable_id', () => {
    installFetch(withFixture({}));
    render(<AssetsPage param="abc" />);

    expect(screen.getByRole('alert')).toHaveTextContent('"abc" is not a valid asset id.');
  });
});

describe('the job detail view', () => {
  it('job_list_links_a_row_to_its_detail_view', async () => {
    installFetch(
      withFixture({
        '/api/jobs': () => jsonResponse(loadFixture('jobs-page.json')),
        '/api/users': () => jsonResponse(loadFixture('users-page.json')),
      }),
    );
    render(<JobsPage />);

    expect(await screen.findByRole('link', { name: '1' })).toHaveAttribute('href', '#/jobs/1');
  });

  it('job_update_sends_an_empty_body_because_the_adapter_hydrates_no_member', async () => {
    const fetchMock = installFetch(
      withFixture({
        '/api/jobs/1': () => jsonResponse(job()),
      }),
    );
    render(<JobsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Update this job' }));
    expect(
      await screen.findByText(
        'A job is written by its dispatcher, so an update changes no member. The request finds the row and reads it back.',
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Update' }));

    const patch = await waitFor(() => sent(fetchMock, '/api/jobs/1', 'PATCH'));
    expect(patch.init?.method).toBe('PATCH');
    expect(requestBody(patch)).toEqual({});
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
  });

  it('job_delete_sends_delete_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(
      withFixture({
        '/api/jobs/1': (_url, init) =>
          jsonResponse(init?.method === 'DELETE' ? detached(job()) : job()),
      }),
    );
    render(<JobsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this job' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Job #1' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/jobs/1').filter(
          (request) => request.init?.method === 'DELETE',
        ),
      ).toHaveLength(1);
    });
    expect(window.location.hash).toBe('#/jobs');
  });

  it('job_detail_reports_an_unparseable_id', () => {
    installFetch(withFixture({}));
    render(<JobsPage param="abc" />);

    expect(screen.getByRole('alert')).toHaveTextContent('"abc" is not a valid job id.');
  });

  it('job_detail_renders_an_absent_status_and_started', async () => {
    installFetch(
      withFixture({
        '/api/jobs/1': () => jsonResponse(jobNoStatus()),
      }),
    );
    render(<JobsPage param="1" />);

    expect(await screen.findByText('[Unknown]')).toBeInTheDocument();
    expect(screen.getByText('[Never]')).toBeInTheDocument();
  });
});
