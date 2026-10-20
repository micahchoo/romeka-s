/**
 * The `repeat` and `pageList` field controls, through a live `BlockForm`: a
 * repeat entry writes the whole record list, and the `listOfPages` tree writes
 * the JSON string the block stores, with the site's pages as its options.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { DraftAction } from '../src/pages/sites/editor/draft';
import { actionsOf, BlockFormHarness, captureBlock, dispatchSpy } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, type Handler } from './stub';

const handler: Handler = (url) => {
  if (url.pathname === '/api/site_pages') {
    return jsonResponse([loadFixture('site-page-1.json'), loadFixture('site-page-2-private.json')]);
  }
  return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

const renderHarness = (layout: string, data: unknown) => {
  const dispatch = dispatchSpy();
  const captured = captureBlock();
  render(
    <BlockFormHarness layout={layout} data={data} onAction={dispatch} onBlock={captured.onBlock} />,
  );
  return { dispatch, key: captured.key() };
};

const action = (dispatch: ReturnType<typeof dispatchSpy>): DraftAction => {
  const first = actionsOf(dispatch)[0];
  if (first === undefined) {
    throw new Error('expected a dispatched action');
  }
  return first;
};

describe('the repeat field control', () => {
  it('adds an empty record', () => {
    const { dispatch, key } = renderHarness('asset', { attachments: [] });

    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));

    expect(action(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [{}],
    });
  });

  it('a nested text edit writes the whole record list', () => {
    const entry = { id: '', page: '', alt_link_title: '', caption: '' };
    const { dispatch, key } = renderHarness('asset', { attachments: [entry] });

    fireEvent.change(screen.getByLabelText('Link text'), {
      target: { value: 'Read more' },
    });

    expect(action(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [{ ...entry, alt_link_title: 'Read more' }],
    });
  });

  it('moves a record up with its arrow button', () => {
    const first = { id: '1', page: '', alt_link_title: 'one', caption: '' };
    const second = { id: '2', page: '', alt_link_title: 'two', caption: '' };
    const { dispatch, key } = renderHarness('asset', { attachments: [first, second] });

    fireEvent.click(screen.getByRole('button', { name: 'Move item 2 up' }));

    expect(action(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [second, first],
    });
  });

  it('moves a record picked up by its handle with the arrow keys', () => {
    const first = { id: '1', page: '', alt_link_title: 'one', caption: '' };
    const second = { id: '2', page: '', alt_link_title: 'two', caption: '' };
    const { dispatch, key } = renderHarness('asset', { attachments: [first, second] });

    const handle = screen.getByRole('button', { name: 'Move item 1, 1 of 2' });
    fireEvent.click(handle);
    fireEvent.keyDown(handle, { key: 'ArrowDown' });

    expect(action(dispatch)).toEqual({
      type: 'setBlockData',
      blockKey: key,
      data: [second, first],
    });
  });
});

describe('the page list field control', () => {
  it('moves a subpage among its siblings and leaves the level above alone', () => {
    const child = (text: string, id: number) => ({ text, data: { id }, children: [] });
    const stored = JSON.stringify([
      { text: 'Welcome', data: { id: 1 }, children: [child('Alpha', 2), child('Beta', 3)] },
      child('Last', 4),
    ]);
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: stored });

    expect(screen.getByRole('button', { name: 'Move Beta, 2 of 2' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Move Beta up' }));

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: JSON.stringify([
        { text: 'Welcome', data: { id: 1 }, children: [child('Beta', 3), child('Alpha', 2)] },
        child('Last', 4),
      ]),
    });
  });

  it('moves a top-level page picked up by its handle', () => {
    const child = (text: string, id: number) => ({ text, data: { id }, children: [] });
    const stored = JSON.stringify([child('Welcome', 1), child('Last', 4)]);
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: stored });

    const handle = screen.getByRole('button', { name: 'Move Last, 2 of 2' });
    fireEvent.click(handle);
    fireEvent.keyDown(handle, { key: 'ArrowUp' });

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: JSON.stringify([child('Last', 4), child('Welcome', 1)]),
    });
  });

  it('adds an empty page node as the stored JSON string', () => {
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: '' });

    fireEvent.click(screen.getByRole('button', { name: 'Add page' }));

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: '[{"text":"","data":{"id":""},"children":[]}]',
    });
  });

  it('choosing a page writes its id and its title', async () => {
    installFetch(handler);
    const stored = JSON.stringify([{ text: 'Welcome', data: { id: 1 }, children: [] }]);
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: stored });

    const select = screen.getByLabelText('Page');
    await waitFor(() => {
      expect(select).toHaveTextContent('Draft');
    });
    fireEvent.change(select, { target: { value: '2' } });

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: JSON.stringify([{ text: 'Draft', data: { id: 2 }, children: [] }]),
    });
  });

  it('adds a subpage under a node', async () => {
    installFetch(handler);
    const stored = JSON.stringify([{ text: 'Welcome', data: { id: 1 }, children: [] }]);
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: stored });

    fireEvent.click(await screen.findByRole('button', { name: 'Add subpage' }));

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: JSON.stringify([
        { text: 'Welcome', data: { id: 1 }, children: [{ text: '', data: { id: '' }, children: [] }] },
      ]),
    });
  });

  it('keeps a stored node url the renderer prefers', async () => {
    installFetch(handler);
    const stored = JSON.stringify([{ text: 'Home', data: { id: 1, url: '/s/main' }, children: [] }]);
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: stored });

    fireEvent.click(await screen.findByRole('button', { name: 'Remove page 1' }));

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: '[]',
    });
  });

  it('reports a tree that is not valid JSON and can clear it', () => {
    const { dispatch, key } = renderHarness('listOfPages', { pagelist: 'nope' });

    expect(screen.getByRole('alert')).toHaveTextContent('The stored page list is not valid JSON.');
    fireEvent.click(screen.getByRole('button', { name: 'Clear the page list' }));

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'pagelist',
      value: '[]',
    });
  });
});
