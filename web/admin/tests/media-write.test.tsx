/**
 * Media write test: the edit panel on the media show page.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { MediaPage } from '../src/pages/media/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const mediaFixture = (): Record<string, unknown> => {
  const fixture = loadFixture('media-951.json');
  if (!isRecord(fixture)) {
    throw new Error('expected a media fixture');
  }
  return fixture;
};

describe('media edit form', () => {
  it('media_edit_puts_alt_text_language_and_the_shared_columns', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/media/951' && init?.method === 'PUT') {
        return jsonResponse({
          ...mediaFixture(),
          'o:alt_text': 'A caption',
          'o:lang': 'en',
        });
      }
      if (url.pathname === '/api/media/951') {
        return jsonResponse(mediaFixture());
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<MediaPage param="951" />);

    fireEvent.click(await screen.findByText('Edit this media'));
    const altText = await screen.findByLabelText('Alt text');
    expect(altText).toHaveValue('');
    expect(screen.getByLabelText('Public')).toBeChecked();

    fireEvent.change(altText, { target: { value: 'A caption' } });
    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'en' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/media/951').filter(
          (request) => request.init?.method === 'PUT',
        ),
      ).toHaveLength(1);
    });
    const put = requestsFor(fetchMock, '/api/media/951').find(
      (request) => request.init?.method === 'PUT',
    );
    if (put === undefined) {
      throw new Error('expected a PUT to /api/media/951');
    }
    expect(put.init?.method).toBe('PUT');
    expect(requestBody(put)).toEqual({
      'o:is_public': true,
      'o:resource_class': { 'o:id': null },
      'o:resource_template': { 'o:id': null },
      'o:thumbnail': { 'o:id': null },
      'o:lang': 'en',
      'o:alt_text': 'A caption',
    });

    expect(await screen.findByText('A caption', { selector: 'dd' })).toBeInTheDocument();
  });
});
