/**
 * The `attachments` field control, through a live `BlockForm` over the `media`
 * layout: the row edits (item, media, caption, order, remove) each dispatch the
 * whole attachment list, and the media select is limited to the chosen item.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { DraftAction } from '../src/pages/sites/editor/draft';
import {
  actionsOf,
  BlockFormHarness,
  captureBlock,
  dispatchSpy,
  rawAttachment,
} from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, type Handler } from './stub';

const handler: Handler = (url) => {
  switch (url.pathname) {
    case '/api/items':
      return listResponse(loadFixture('items-page.json'), 2);
    case '/api/items/5':
      return jsonResponse(loadFixture('item-1.json'));
    case '/api/media':
      return listResponse(loadFixture('media-page.json'), 2);
    default:
      return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  }
};

const renderAttachments = (attachments: readonly unknown[]) => {
  const dispatch = dispatchSpy();
  const captured = captureBlock();
  render(
    <BlockFormHarness
      layout="media"
      data={{}}
      extra={{ 'o:attachment': attachments }}
      onAction={dispatch}
      onBlock={captured.onBlock}
    />,
  );
  return { dispatch, key: captured.key() };
};

const attachmentAction = (dispatch: ReturnType<typeof dispatchSpy>): DraftAction => {
  const action = actionsOf(dispatch)[0];
  if (action === undefined) {
    throw new Error('expected a dispatched action');
  }
  return action;
};

describe('the attachments field control', () => {
  it('shows the stored item, media and caption', async () => {
    installFetch(handler);
    renderAttachments([rawAttachment(5, 7, 'A caption')]);

    expect(await screen.findByText('Fixture Item One')).toBeInTheDocument();
    const caption = screen.getByRole('textbox', { name: 'Caption' });
    expect(caption).toHaveTextContent('A caption');
    await waitFor(() => {
      expect(screen.getByLabelText('Media')).toHaveTextContent('Fixture photo');
    });
  });

  it('falls back to the item id when the stored item cannot be read', async () => {
    installFetch((url) => {
      if (url.pathname === '/api/items/5') {
        return jsonResponse({ errors: { error: 'not found' } }, { status: 404 });
      }
      return handler(url, undefined);
    });
    renderAttachments([rawAttachment(5, null, '')]);

    expect(await screen.findByText('Item 5')).toBeInTheDocument();
  });

  it('adds an empty attachment row', () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([]);

    fireEvent.click(screen.getByRole('button', { name: 'Add attachment' }));

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [{ item: null, media: null, caption: '' }],
    });
  });

  it('a picked item clears the media and writes the list', async () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([rawAttachment(null, null, '')]);

    fireEvent.change(screen.getByLabelText('Item'), { target: { value: 'cat' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Fixture Item One' }));

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [{ item: 12, media: null, caption: '' }],
    });
  });

  it('the media select writes the chosen media', async () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([rawAttachment(5, null, '')]);

    const media = await screen.findByLabelText('Media');
    await waitFor(() => {
      expect(media).toHaveTextContent('Fixture photo');
    });
    fireEvent.change(media, { target: { value: '7' } });

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [{ item: 5, media: 7, caption: '' }],
    });
  });

  it('a caption edit writes the whole list with the new caption', () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([rawAttachment(5, 7, '')]);

    const caption = screen.getByRole('textbox', { name: 'Caption' });
    caption.innerHTML = '<p>New caption</p>';
    fireEvent.input(caption);

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [{ item: 5, media: 7, caption: '<p>New caption</p>' }],
    });
  });

  it('moving a row swaps it with its neighbour', () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([
      rawAttachment(5, null, 'first'),
      rawAttachment(12, null, 'second'),
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Move attachment 2 up' }));

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [
        { item: 12, media: null, caption: 'second' },
        { item: 5, media: null, caption: 'first' },
      ],
    });
  });

  it('removing a row drops it from the list', () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([
      rawAttachment(5, null, 'first'),
      rawAttachment(12, null, 'second'),
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Remove attachment 1' }));

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [{ item: 12, media: null, caption: 'second' }],
    });
  });

  it('reports a failed item search', async () => {
    installFetch((url) => {
      if (url.pathname === '/api/items') {
        return jsonResponse({ errors: { error: 'search down' } }, { status: 500 });
      }
      return handler(url, undefined);
    });
    renderAttachments([rawAttachment(null, null, '')]);

    fireEvent.change(screen.getByLabelText('Item'), { target: { value: 'cat' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The item search failed. search down (HTTP 500)',
    );
  });
});
