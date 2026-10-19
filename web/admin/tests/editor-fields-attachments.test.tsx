/**
 * The `attachments` field control, through a live `BlockForm` over the `media`
 * layout: picking or unpicking an item in the shared picker adds or removes its
 * row, and the row edits (media, caption, order, remove) each dispatch the
 * whole attachment list. The media choice is limited to the row's item.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
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
    case '/api/items/12':
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

/** The block's own rows, apart from the picker's result grid. */
const rows = () => screen.getByRole('list', { name: 'Items to show' });

describe('the attachments field control', () => {
  it('shows the stored item, media and caption', async () => {
    installFetch(handler);
    renderAttachments([rawAttachment(5, 7, 'A caption')]);

    expect(await within(rows()).findByText('Fixture Item One')).toBeInTheDocument();
    const caption = screen.getByRole('textbox', { name: 'Caption' });
    expect(caption).toHaveTextContent('A caption');
    expect(await screen.findByRole('radio', { name: 'Fixture photo' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Main media' })).not.toBeChecked();
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

  it('picking an item in the picker appends its row', async () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([rawAttachment(5, null, 'kept')]);

    const pick = await screen.findByRole('button', { name: 'Fixture Item One', pressed: false });
    fireEvent.click(pick);

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [
        { item: 5, media: null, caption: 'kept' },
        { item: 12, media: null, caption: '' },
      ],
    });
  });

  it('unpicking an item in the picker removes its row', async () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([
      rawAttachment(12, null, 'gone'),
      rawAttachment(5, null, 'kept'),
    ]);

    const picked = await screen.findByRole('button', { name: 'Fixture Item One', pressed: true });
    fireEvent.click(picked);

    expect(attachmentAction(dispatch)).toEqual({
      type: 'setAttachments',
      blockKey: key,
      attachments: [{ item: 5, media: null, caption: 'kept' }],
    });
  });

  it('a media thumbnail writes the chosen media', async () => {
    installFetch(handler);
    const { dispatch, key } = renderAttachments([rawAttachment(5, null, '')]);

    fireEvent.click(await screen.findByRole('radio', { name: 'Fixture photo' }));

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
    renderAttachments([]);

    expect(await screen.findByText(/Search failed: .*search down/)).toBeInTheDocument();
  });
});
