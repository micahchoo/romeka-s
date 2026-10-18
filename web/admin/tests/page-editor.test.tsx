/**
 * The page editor's save path. A loaded block keeps its attachments in the PUT
 * body, because a full write that omits `o:attachment` clears every stored
 * attachment on the server. The editor never edits attachments, so the test
 * saves without touching the block.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { SitesPage } from '../src/pages/sites/page';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const page = {
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/site_pages/1',
  '@type': ['o:SitePage'],
  'o:id': 1,
  'o:slug': 'welcome',
  'o:title': 'Welcome',
  'o:is_public': true,
  'o:layout': null,
  'o:layout_data': [],
  'o:block': [
    {
      'o:layout': 'media',
      'o:data': { layout: 'horizontal' },
      'o:layout_data': null,
      'o:attachment': [
        {
          'o:item': { '@id': 'http://localhost/api/items/5', 'o:id': 5 },
          'o:media': { '@id': 'http://localhost/api/media/9', 'o:id': 9 },
          'o:caption': 'First',
        },
        {
          'o:item': { '@id': 'http://localhost/api/items/6', 'o:id': 6 },
          'o:media': null,
          'o:caption': 'Second',
        },
      ],
    },
  ],
  'o:site': { '@id': 'http://localhost/api/sites/1', 'o:id': 1 },
  'o:created': {
    '@value': '2024-01-02T10:00:00+00:00',
    '@type': 'http://www.w3.org/2001/XMLSchema#dateTime',
  },
  'o:modified': null,
};

describe('the page editor save', () => {
  it('carries a loaded block attachments through to the PUT body', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/site_pages/1' && init?.method === 'PUT') {
        return jsonResponse(page);
      }
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(page);
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved.')).toBeInTheDocument();

    const put = first(
      requestsFor(fetchMock, '/api/site_pages/1').filter(
        (request) => request.init?.method === 'PUT',
      ),
    );
    const blocks = requestBody(put)['o:block'];
    expect(isUnknownArray(blocks)).toBe(true);
    if (isUnknownArray(blocks)) {
      expect(blocks).toHaveLength(1);
      const block = blocks[0];
      expect(isRecord(block)).toBe(true);
      if (isRecord(block)) {
        expect(block['o:attachment']).toEqual([
          { 'o:item': { 'o:id': 5 }, 'o:media': { 'o:id': 9 }, 'o:caption': 'First' },
          { 'o:item': { 'o:id': 6 }, 'o:media': null, 'o:caption': 'Second' },
        ]);
      }
    }
  });
});
