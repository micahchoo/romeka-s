/**
 * The page editor repairs, through the route a curator opens: the page
 * settings come back after a selection, Undo and Redo live in the toolbar with
 * their shortcuts, the Keys popover lists the hidden keys, a removed block can
 * be brought back from its toast, and a group lists its blocks as slots.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const PAGE = '/api/site_pages/1';

const block = (layout: string, data: unknown): Record<string, unknown> => ({
  'o:layout': layout,
  'o:data': data,
  'o:layout_data': null,
  'o:attachment': [],
});

/** Two HTML blocks, the second inside a group of one. */
const PAGE_BODY = {
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
    block('html', { html: '<p>First block.</p>' }),
    block('blockGroup', { span: 1 }),
    block('html', { html: '<p>Member block.</p>' }),
  ],
  'o:site': { '@id': 'http://localhost/api/sites/1', 'o:id': 1 },
  'o:created': {
    '@value': '2024-01-02T10:00:00+00:00',
    '@type': 'http://www.w3.org/2001/XMLSchema#dateTime',
  },
  'o:modified': null,
};

const handler = (url: URL): Response => {
  if (url.pathname === '/s/my-site/preview') {
    return new Response('<html><body><p>First block.</p></body></html>', {
      status: 200,
      headers: { 'Content-Type': 'text/html' },
    });
  }
  if (url.pathname === PAGE) {
    return jsonResponse(PAGE_BODY);
  }
  if (url.pathname === '/api/sites/1') {
    return jsonResponse(loadFixture('site-1.json'));
  }
  if (url.pathname === '/api/site_pages') {
    return listResponse([loadFixture('site-page-1.json')], 1);
  }
  if (url.pathname === '/api/item_sets') {
    return listResponse(loadFixture('item-sets-page.json'), 2);
  }
  if (url.pathname === '/api/resource_classes') {
    return listResponse(loadFixture('resource-classes-page.json'), 2);
  }
  return jsonResponse(
    { errors: { error: `Unexpected request to ${url.pathname}` } },
    { status: 404 },
  );
};

const FIRST = /Block 1: HTML/;

const renderEditor = async (): Promise<void> => {
  installFetch(handler);
  render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
  await screen.findByRole('listitem', { name: FIRST });
};

const selectFirst = async (): Promise<HTMLElement> => {
  fireEvent.click(screen.getByRole('listitem', { name: FIRST }));
  const inspector = screen.getByRole('region', { name: 'Block inspector' });
  await within(inspector).findByRole('textbox', { name: 'HTML' });
  return inspector;
};

const pageSettingsShown = (): boolean => screen.queryByLabelText('Title') !== null;

const toolbar = (): HTMLElement => screen.getByRole('region', { name: 'Editor actions' });

const title = (): HTMLElement => screen.getByLabelText('Title');

afterEach(() => {
  window.location.hash = '';
  window.localStorage.removeItem('romeka.editor.outline');
});

describe('the page settings come back after a selection (R1)', () => {
  it('names the selection in a crumb whose Page button shows the page settings', async () => {
    await renderEditor();
    const inspector = await selectFirst();
    expect(pageSettingsShown()).toBe(false);

    const crumb = within(inspector).getByRole('navigation', {
      name: 'Selection',
    });
    expect(crumb).toHaveTextContent('HTML');
    fireEvent.click(within(crumb).getByRole('button', { name: 'Page' }));

    expect(title()).toHaveValue('Welcome');
  });

  it('shows the page settings on Escape outside a text field', async () => {
    await renderEditor();
    await selectFirst();

    fireEvent.keyDown(document.body, { key: 'Escape' });

    expect(title()).toHaveValue('Welcome');
  });

  it('leaves Escape to a text field that has focus', async () => {
    await renderEditor();
    const inspector = await selectFirst();

    fireEvent.keyDown(within(inspector).getByRole('textbox', { name: 'HTML' }), { key: 'Escape' });

    expect(pageSettingsShown()).toBe(false);
  });

  it('shows the page settings on a click on empty canvas space, not on a card', async () => {
    await renderEditor();
    await selectFirst();
    fireEvent.click(
      within(screen.getByRole('listitem', { name: FIRST })).getByText('First block.'),
    );
    expect(pageSettingsShown()).toBe(false);

    fireEvent.click(screen.getByRole('region', { name: 'Page canvas' }));

    expect(title()).toHaveValue('Welcome');
  });
});

describe('Undo and Redo in the toolbar (R7)', () => {
  it('stays in the toolbar with the Outline closed and undoes an edit', async () => {
    window.localStorage.setItem('romeka.editor.outline', 'closed');
    installFetch(handler);
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);
    await screen.findByLabelText('Title');
    await waitFor(() => {
      expect(screen.queryByRole('region', { name: 'Outline' })).toBeNull();
    });

    const undo = within(toolbar()).getByRole('button', { name: 'Undo' });
    const redo = within(toolbar()).getByRole('button', { name: 'Redo' });
    expect(undo).toBeDisabled();

    fireEvent.change(title(), { target: { value: 'Changed' } });
    fireEvent.click(undo);
    expect(title()).toHaveValue('Welcome');
    fireEvent.click(redo);
    expect(title()).toHaveValue('Changed');
  });

  it('undoes on Ctrl+Z and redoes on Shift+Ctrl+Z outside a text field', async () => {
    await renderEditor();
    fireEvent.change(title(), { target: { value: 'Changed' } });

    fireEvent.keyDown(document.body, { key: 'z', ctrlKey: true });
    expect(title()).toHaveValue('Welcome');

    fireEvent.keyDown(document.body, {
      key: 'Z',
      ctrlKey: true,
      shiftKey: true,
    });
    expect(title()).toHaveValue('Changed');

    fireEvent.keyDown(document.body, { key: 'z', metaKey: true });
    expect(title()).toHaveValue('Welcome');
  });

  it('leaves Ctrl+Z inside a text input to the browser', async () => {
    await renderEditor();
    fireEvent.change(title(), { target: { value: 'Changed' } });

    fireEvent.keyDown(title(), { key: 'z', ctrlKey: true });

    expect(title()).toHaveValue('Changed');
  });
});

describe('the hidden keys', () => {
  it('lists the editor shortcuts in the Keys popover, and Escape closes only the popover', async () => {
    await renderEditor();
    await selectFirst();

    const button = within(toolbar()).getByRole('button', { name: 'Keys' });
    expect(button).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(button);

    const popover = screen.getByRole('dialog', { name: 'Keyboard shortcuts' });
    for (const action of [
      'Save the page',
      'Undo',
      'Redo',
      'Remove the focused block',
      'Pick up the focused block, move it, then drop it with Space',
      'Resize the focused block in a grid',
      'Show the page settings',
    ]) {
      expect(within(popover).getByText(action)).toBeInTheDocument();
    }
    expect(popover).toHaveTextContent('Shift');
    expect(popover).toHaveTextContent('Esc');

    fireEvent.keyDown(popover, { key: 'Escape' });

    expect(screen.queryByRole('dialog', { name: 'Keyboard shortcuts' })).toBeNull();
    expect(pageSettingsShown()).toBe(false);
  });

  it('removes a focused card on Delete and brings it back from the toast', async () => {
    await renderEditor();
    const card = screen.getByRole('listitem', { name: FIRST });

    fireEvent.keyDown(card, { key: 'Delete' });

    expect(screen.queryByText('First block.')).toBeNull();
    const toast = screen.getByText('Removed HTML.').closest('.editor-toast');
    if (!(toast instanceof HTMLElement)) {
      throw new Error('no toast');
    }
    expect(toast).toHaveAttribute('role', 'status');
    fireEvent.click(within(toast).getByRole('button', { name: 'Undo' }));

    expect(screen.getByText('First block.')).toBeInTheDocument();
    expect(screen.queryByText('Removed HTML.')).toBeNull();
  });

  it('drops the toast once a later edit makes its Undo mean something else', async () => {
    await renderEditor();
    fireEvent.keyDown(screen.getByRole('listitem', { name: FIRST }), {
      key: 'Backspace',
    });
    expect(screen.getByText('Removed HTML.')).toBeInTheDocument();

    fireEvent.change(title(), { target: { value: 'Changed' } });

    expect(screen.queryByText('Removed HTML.')).toBeNull();
  });

  it('gives every card a grab handle', async () => {
    await renderEditor();
    const card = screen.getByRole('listitem', { name: FIRST });
    expect(within(card).getByTitle('Drag to move')).toHaveTextContent('⠿');
  });
});

describe('a group lists its blocks as slots (R6)', () => {
  it('shows each block in its slot, with no count to type into', async () => {
    await renderEditor();
    fireEvent.click(screen.getByRole('listitem', { name: /Block 2: Block group/ }));
    const inspector = screen.getByRole('region', { name: 'Block inspector' });

    const slot = await within(inspector).findByRole('button', { name: /Slot 1/ });
    expect(slot).toHaveTextContent('HTML');
    expect(inspector).toHaveTextContent('1 block');
    expect(within(inspector).queryByRole('spinbutton')).toBeNull();
    expect(within(inspector).queryByLabelText('Blocks in the group')).toBeNull();
  });
});
