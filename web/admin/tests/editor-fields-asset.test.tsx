/**
 * The `assetRef` and `pageRef` field controls, through a live `BlockForm` over
 * the `asset` layout: the picker lists `/api/assets` as thumbnails and names,
 * selecting one writes the id into the repeat entry, the upload sends the
 * multipart body the server parses, and an upload failure renders.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import type { DraftAction } from '../src/pages/sites/editor/draft';
import { actionsOf, BlockFormHarness, captureBlock, dispatchSpy } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestsFor, type CapturedRequest, type Handler } from './stub';

const assets = (): readonly Record<string, unknown>[] => {
  const page = loadFixture('assets-page.json');
  if (!Array.isArray(page)) {
    throw new Error('expected an asset list fixture');
  }
  return page.filter(isRecord);
};

const firstAsset = (): Record<string, unknown> => {
  const [asset] = assets();
  if (asset === undefined) {
    throw new Error('expected an asset fixture');
  }
  return asset;
};

const sitePages = (): readonly unknown[] => [
  loadFixture('site-page-1.json'),
  loadFixture('site-page-2-private.json'),
];

const entry = (extra: Readonly<Record<string, unknown>> = {}): Readonly<Record<string, unknown>> => ({
  id: '',
  page: '',
  alt_link_title: '',
  caption: '',
  ...extra,
});

const renderAsset = (
  attachment: Readonly<Record<string, unknown>>,
  handler: Handler,
) => {
  const fetchMock = installFetch(handler);
  const dispatch = dispatchSpy();
  const captured = captureBlock();
  render(
    <BlockFormHarness
      layout="asset"
      data={{ attachments: [attachment] }}
      onAction={dispatch}
      onBlock={captured.onBlock}
    />,
  );
  return { dispatch, fetchMock, key: captured.key() };
};

const listHandler = (post: Handler): Handler => (url, init) => {
  if (url.pathname === '/api/assets' && init?.method === 'POST') {
    return post(url, init);
  }
  switch (url.pathname) {
    case '/api/assets':
      return jsonResponse(loadFixture('assets-page.json'));
    case '/api/assets/1':
      return jsonResponse(firstAsset());
    case '/api/site_pages':
      return jsonResponse(sitePages());
    default:
      return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  }
};

const attachmentAction = (dispatch: ReturnType<typeof dispatchSpy>): DraftAction => {
  const action = actionsOf(dispatch)[0];
  if (action === undefined) {
    throw new Error('expected a dispatched action');
  }
  return action;
};

describe('the asset picker', () => {
  it('lists the assets and writes the chosen id into the entry', async () => {
    const { dispatch, key } = renderAsset(entry(), listHandler(() => jsonResponse(firstAsset())));

    expect(await screen.findByText('[No asset]')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Choose asset' }));
    fireEvent.click(await screen.findByRole('button', { name: 'omeka-logo.png' }));

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [entry({ id: '1' })],
    });
  });

  it('shows the stored asset once the entry names one', async () => {
    renderAsset(entry({ id: '1' }), listHandler(() => jsonResponse(firstAsset())));

    expect(await screen.findByText('omeka-logo.png')).toBeInTheDocument();
  });

  it('falls back to the id when the stored asset cannot be read', async () => {
    renderAsset(entry({ id: '1' }), (url) => {
      if (url.pathname === '/api/assets/1') {
        return jsonResponse({ errors: { error: 'not found' } }, { status: 404 });
      }
      return listHandler(() => jsonResponse(firstAsset()))(url, undefined);
    });

    expect(await screen.findByText('Asset 1')).toBeInTheDocument();
  });

  it('uploads a file as the multipart body the server parses', async () => {
    const created = { ...firstAsset(), 'o:id': 3, 'o:name': 'new.png' };
    const { dispatch, fetchMock, key } = renderAsset(entry(), listHandler(() => jsonResponse(created)));
    const file = new File(['bytes'], 'new.png', { type: 'image/png' });

    fireEvent.change(await screen.findByLabelText('Upload'), { target: { files: [file] } });

    const request: CapturedRequest = await waitFor(() => {
      const sent = requestsFor(fetchMock, '/api/assets').filter(
        (entry) => entry.init?.method === 'POST',
      );
      const first = sent[0];
      if (first === undefined) {
        throw new Error('expected the upload request');
      }
      return first;
    });
    const body = request.init?.body;
    expect(body).toBeInstanceOf(FormData);
    if (!(body instanceof FormData)) {
      throw new Error('expected a FormData body');
    }
    expect(body.get('data')).toBe(JSON.stringify({ 'o:name': 'new.png', 'o:alt_text': null }));
    expect(body.get('file')).toBe(file);
    // No Content-Type by hand: the browser sets it with the boundary.
    expect(request.init?.headers).toEqual({ Accept: 'application/json' });
    expect(attachmentAction(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [entry({ id: '3' })],
    });
  });

  it('renders the API message when the upload fails', async () => {
    const { dispatch } = renderAsset(
      entry(),
      listHandler(() =>
        jsonResponse({ errors: { file: ['No file was uploaded'] } }, { status: 422 }),
      ),
    );
    const file = new File(['bytes'], 'note.txt', { type: 'text/plain' });

    fireEvent.change(await screen.findByLabelText('Upload'), { target: { files: [file] } });

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The upload failed. No file was uploaded (HTTP 422)',
    );
    expect(dispatch).not.toHaveBeenCalled();
  });
});

describe('the page reference control', () => {
  it('lists the site pages and writes the chosen id into the entry', async () => {
    const { dispatch, key } = renderAsset(entry(), listHandler(() => jsonResponse(firstAsset())));

    const select = await screen.findByRole('combobox', { name: 'A page' });
    await waitFor(() => {
      expect(select).toHaveTextContent('Welcome');
    });
    fireEvent.change(select, { target: { value: '2' } });

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [entry({ page: '2' })],
    });
  });
});
