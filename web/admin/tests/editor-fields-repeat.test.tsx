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
      type: 'setField',
      blockKey: key,
      field: 'attachments',
      value: [{}],
    });
  });

  it('a nested text edit writes the whole record list', () => {
    const entry = { id: '', page: '', alt_link_title: '', caption: '' };
    const { dispatch, key } = renderHarness('asset', { attachments: [entry] });

    fireEvent.change(screen.getByLabelText('Alternative link title'), {
      target: { value: 'Read more' },
    });

    expect(action(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'attachments',
      value: [{ ...entry, alt_link_title: 'Read more' }],
    });
  });
});

describe('the page list field control', () => {
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
