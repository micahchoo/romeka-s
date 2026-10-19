/**
 * The page canvas, driven through its controls. Each case asserts the
 * `DraftAction` a curator's gesture produces, and where it matters the card
 * order or selection the draft then renders, so the test breaks when the
 * gesture stops reaching the model.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { useReducer, useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { draftReducer } from '../src/pages/sites/editor/draft';
import type { BlockKey, DraftAction, PageDraft } from '../src/pages/sites/editor/draft';
import { PageCanvas } from '../src/pages/sites/editor/canvas';
import { blockAt, keyOf, load, rawBlock } from './editor-draft-helpers';

const card = (key: string): HTMLElement => {
  const element = document.querySelector(`[data-key="${key}"]`);
  if (!(element instanceof HTMLElement)) {
    throw new Error(`no card for ${key}`);
  }
  return element;
};

const cardKeys = (): readonly string[] =>
  [...document.querySelectorAll('li.canvas-card')].map(
    (element) => element.getAttribute('data-key') ?? '',
  );

/** The canvas with a reducer behind it, so an action shows up in the cards. */
const Harness = ({
  initial,
  onAction,
}: {
  readonly initial: PageDraft;
  readonly onAction: (action: DraftAction) => void;
}): ReactElement => {
  const [draft, dispatch] = useReducer(draftReducer, initial);
  const [selected, setSelected] = useState<BlockKey | null>(null);
  return (
    <PageCanvas
      draft={draft}
      siteSettings={null}
      selectedKey={selected}
      onSelect={setSelected}
      dispatch={(action) => {
        onAction(action);
        dispatch(action);
      }}
    />
  );
};

const renderCanvas = (
  draft: PageDraft,
): { readonly dispatch: ReturnType<typeof vi.fn>; readonly onSelect: ReturnType<typeof vi.fn> } => {
  const dispatch = vi.fn();
  const onSelect = vi.fn();
  render(<PageCanvas draft={draft} siteSettings={null} selectedKey={null} onSelect={onSelect} dispatch={dispatch} />);
  return { dispatch, onSelect };
};

const renderHarness = (initial: PageDraft): readonly DraftAction[] => {
  const actions: DraftAction[] = [];
  render(
    <Harness
      initial={initial}
      onAction={(action) => {
        actions.push(action);
      }}
    />,
  );
  return actions;
};

const gridPage = (blocks: readonly unknown[]): PageDraft =>
  load(blocks, { 'o:layout': 'grid', 'o:layout_data': { grid_columns: 12 } });

describe('the canvas cards', () => {
  it('shows a layout label, a content summary and a validation badge', () => {
    const draft = load([
      rawBlock('html', { html: '<p>Hello <strong>world</strong></p>' }),
      rawBlock('pageTitle', []),
      rawBlock('oembed', {}),
    ]);
    renderCanvas(draft);

    const html = card(blockAt(draft.blocks, 0).key);
    expect(html).toHaveTextContent('HTML');
    expect(html).toHaveTextContent('Hello world');

    // The pageTitle block renders the page's own title, not a stored field.
    expect(card(blockAt(draft.blocks, 1).key)).toHaveTextContent('Welcome');
    expect(card(blockAt(draft.blocks, 2).key)).toHaveTextContent('1 issues');
  });

  it('counts attachments on a card', () => {
    const draft = load([
      rawBlock('media', {}, {
        'o:attachment': [
          { 'o:item': null, 'o:media': null, 'o:caption': 'one' },
          { 'o:item': null, 'o:media': null, 'o:caption': 'two' },
        ],
      }),
    ]);
    renderCanvas(draft);
    expect(card(blockAt(draft.blocks, 0).key)).toHaveTextContent('2 attachments');
  });

  it('selects a card when it is clicked', () => {
    const draft = load([rawBlock('html', { html: 'a' }), rawBlock('html', { html: 'b' })]);
    const { onSelect } = renderCanvas(draft);
    fireEvent.click(card(blockAt(draft.blocks, 1).key));
    expect(onSelect).toHaveBeenCalledWith(blockAt(draft.blocks, 1).key);
  });

  it('shows the block count in the toolbar and leaves the empty state to the shell', () => {
    renderCanvas(load([]));
    expect(screen.queryByText(/No blocks yet/)).toBeNull();
    expect(screen.getByText('0 blocks')).toBeInTheDocument();
  });

  it('counts the blocks in the toolbar', () => {
    renderCanvas(load([rawBlock('html', { html: 'a' }), rawBlock('html', { html: 'b' })]));
    expect(screen.getByText('2 blocks')).toBeInTheDocument();
  });
});

describe('keyboard reorder', () => {
  it('dispatches moveBlock one position down', () => {
    const draft = load([
      rawBlock('html', { html: 'a' }),
      rawBlock('html', { html: 'b' }),
      rawBlock('html', { html: 'c' }),
    ]);
    const middle = blockAt(draft.blocks, 1).key;
    const actions = renderHarness(draft);

    fireEvent.keyDown(card(middle), { key: ' ' });
    fireEvent.keyDown(card(middle), { key: 'ArrowDown' });
    fireEvent.keyDown(card(middle), { key: ' ' });

    expect(actions).toContainEqual({ type: 'moveBlock', blockKey: middle, toIndex: 2 });
    expect(cardKeys()).toEqual([
      blockAt(draft.blocks, 0).key,
      blockAt(draft.blocks, 2).key,
      middle,
    ]);
  });

  it('dispatches moveBlock one position up', () => {
    const draft = load([
      rawBlock('html', { html: 'a' }),
      rawBlock('html', { html: 'b' }),
      rawBlock('html', { html: 'c' }),
    ]);
    const middle = blockAt(draft.blocks, 1).key;
    const actions = renderHarness(draft);

    fireEvent.keyDown(card(middle), { key: ' ' });
    fireEvent.keyDown(card(middle), { key: 'ArrowUp' });
    fireEvent.keyDown(card(middle), { key: ' ' });

    expect(actions).toContainEqual({ type: 'moveBlock', blockKey: middle, toIndex: 0 });
    expect(cardKeys()).toEqual([
      middle,
      blockAt(draft.blocks, 0).key,
      blockAt(draft.blocks, 2).key,
    ]);
  });

  it('announces the pick-up and leaves the order alone until an arrow', () => {
    const draft = load([rawBlock('html', { html: 'a' }), rawBlock('html', { html: 'b' })]);
    const first = blockAt(draft.blocks, 0).key;
    const actions = renderHarness(draft);

    fireEvent.keyDown(card(first), { key: ' ' });

    expect(actions).toEqual([]);
    expect(screen.getByRole('status')).toHaveTextContent('Picked up HTML');
  });
});

describe('grid gestures', () => {
  it('places a card at its stored position and span', () => {
    const draft = gridPage([
      rawBlock('html', { html: 'a' }, {
        'o:layout_data': { grid_column_position: '3', grid_column_span: '6' },
      }),
    ]);
    renderCanvas(draft);
    expect(card(blockAt(draft.blocks, 0).key)).toHaveStyle({
      gridColumnStart: '3',
      gridColumnEnd: 'span 6',
    });
  });

  it('falls back to auto and the column count for out-of-range values', () => {
    const draft = gridPage([
      rawBlock('html', { html: 'a' }, {
        'o:layout_data': { grid_column_position: '99', grid_column_span: '99' },
      }),
    ]);
    renderCanvas(draft);
    expect(card(blockAt(draft.blocks, 0).key)).toHaveStyle({
      gridColumnStart: 'auto',
      gridColumnEnd: 'span 12',
    });
  });

  it('resizes the span with Shift+ArrowRight and dispatches setLayoutData', () => {
    const draft = gridPage([
      rawBlock('html', { html: 'a' }, { 'o:layout_data': { grid_column_span: '6' } }),
    ]);
    const key = blockAt(draft.blocks, 0).key;
    const { dispatch } = renderCanvas(draft);

    fireEvent.keyDown(card(key), { key: 'ArrowRight', shiftKey: true });

    expect(dispatch).toHaveBeenCalledWith({
      type: 'setLayoutData',
      blockKey: key,
      key: 'grid_column_span',
      value: '7',
    });
  });

  it('sets the column with an arrow once the card is picked up', () => {
    const draft = gridPage([rawBlock('html', { html: 'a' })]);
    const key = blockAt(draft.blocks, 0).key;
    const { dispatch } = renderCanvas(draft);

    fireEvent.keyDown(card(key), { key: ' ' });
    fireEvent.keyDown(card(key), { key: 'ArrowRight' });

    expect(dispatch).toHaveBeenCalledWith({
      type: 'setLayoutData',
      blockKey: key,
      key: 'grid_column_position',
      value: '2',
    });
  });

  it('resizes the span by dragging the card edge', () => {
    const draft = gridPage([
      rawBlock('html', { html: 'a' }, { 'o:layout_data': { grid_column_span: '6' } }),
    ]);
    const key = blockAt(draft.blocks, 0).key;
    const { dispatch } = renderCanvas(draft);

    const handle = screen.getByRole('button', { name: 'Resize block 1 span' });
    fireEvent.pointerDown(handle, { clientX: 100 });
    fireEvent.pointerMove(window, { clientX: 180 });
    fireEvent.pointerUp(window);

    expect(dispatch).toHaveBeenCalledWith({
      type: 'setLayoutData',
      blockKey: key,
      key: 'grid_column_span',
      value: '8',
    });
  });
});

describe('the block picker', () => {
  it('inserts the chosen layout at the index', () => {
    const draft = load([rawBlock('html', { html: 'a' }), rawBlock('html', { html: 'b' })]);
    const { dispatch } = renderCanvas(draft);

    fireEvent.click(screen.getByRole('button', { name: 'Insert block at position 2' }));
    fireEvent.change(screen.getByLabelText('Search layouts'), { target: { value: 'line' } });
    fireEvent.click(screen.getByRole('button', { name: 'Line break' }));

    expect(dispatch).toHaveBeenCalledWith({ type: 'addBlock', layout: 'lineBreak', index: 1 });
  });

  it('says in one line what each block adds', () => {
    renderCanvas(load([rawBlock('html', { html: 'a' })]));
    fireEvent.click(screen.getByRole('button', { name: 'Insert block at position 2' }));

    expect(screen.getByRole('button', { name: 'Browse preview' })).toHaveAccessibleDescription(
      'A list of items, sets or media in this site that match a search.',
    );
  });

  it('offers templates that insert blocks and leaves out Blank, which inserts none', () => {
    renderCanvas(load([rawBlock('html', { html: 'a' })]));
    fireEvent.click(screen.getByRole('button', { name: 'Insert block at position 2' }));
    const picker = screen.getByRole('dialog', { name: 'Insert a block or template' });

    expect(within(picker).getByRole('button', { name: 'Gallery' })).toBeInTheDocument();
    expect(within(picker).queryByRole('button', { name: 'Blank' })).toBeNull();
  });

  it('filters the layouts by the search text', () => {
    renderCanvas(load([rawBlock('html', { html: 'a' })]));
    fireEvent.click(screen.getByRole('button', { name: 'Insert block at position 2' }));
    fireEvent.change(screen.getByLabelText('Search layouts'), { target: { value: 'zzz' } });

    expect(screen.getByText('No layout matches.')).toBeInTheDocument();
  });
});

describe('groups', () => {
  it('moves a top-level block into the group above it', () => {
    const draft = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'inside' }),
      rawBlock('html', { html: 'outside' }),
    ]);
    const group = keyOf(draft, 'blockGroup');
    const outside = blockAt(draft.blocks, 2).key;
    const { dispatch } = renderCanvas(draft);

    fireEvent.click(screen.getByRole('button', { name: 'Actions for block 3' }));
    fireEvent.click(screen.getByRole('button', { name: 'Move into the group above' }));

    expect(dispatch).toHaveBeenCalledWith({
      type: 'moveBlock',
      blockKey: outside,
      toIndex: 2,
      into: group,
    });
  });

  it('moves a member out of its group', () => {
    const draft = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'inside' }),
    ]);
    const member = blockAt(draft.blocks, 1).key;
    const { dispatch } = renderCanvas(draft);

    fireEvent.click(screen.getByRole('button', { name: 'Actions for block 2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Move out of the group' }));

    expect(dispatch).toHaveBeenCalledWith({
      type: 'moveBlock',
      blockKey: member,
      toIndex: 2,
      into: null,
    });
  });

  it('drops a dragged block into the group', () => {
    const draft = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'inside' }),
      rawBlock('html', { html: 'outside' }),
    ]);
    const group = keyOf(draft, 'blockGroup');
    const outside = blockAt(draft.blocks, 2).key;
    const { dispatch } = renderCanvas(draft);

    const transfer = { setData: vi.fn(), effectAllowed: '' };
    fireEvent.dragStart(card(outside), { dataTransfer: transfer });
    const zone = document.querySelector(`[data-drop-into="${group}"]`);
    expect(zone).not.toBeNull();
    fireEvent.drop(zone ?? document.body, { dataTransfer: transfer });

    expect(dispatch).toHaveBeenCalledWith({
      type: 'moveBlock',
      blockKey: outside,
      toIndex: 2,
      into: group,
    });
  });
});

describe('delete', () => {
  it('removes a block from its card menu', () => {
    const draft = load([rawBlock('html', { html: 'a' })]);
    const key = blockAt(draft.blocks, 0).key;
    const { dispatch } = renderCanvas(draft);

    fireEvent.click(screen.getByRole('button', { name: 'Actions for block 1' }));
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));

    expect(dispatch).toHaveBeenCalledWith({ type: 'removeBlock', blockKey: key });
  });

  it('removes the focused block on Delete', () => {
    const draft = load([rawBlock('html', { html: 'a' })]);
    const key = blockAt(draft.blocks, 0).key;
    const { dispatch } = renderCanvas(draft);

    fireEvent.keyDown(card(key), { key: 'Delete' });

    expect(dispatch).toHaveBeenCalledWith({ type: 'removeBlock', blockKey: key });
  });
});

describe('undo and redo', () => {
  it('walks back and forward through an edit', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    renderHarness(start);

    expect(screen.getByRole('button', { name: 'Undo' })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Insert block at position 2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Line break' }));
    expect(cardKeys()).toHaveLength(2);

    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(cardKeys()).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: 'Redo' }));
    expect(cardKeys()).toHaveLength(2);
  });

  it('runs undo and redo from the keyboard', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    const edited = draftReducer(start, { type: 'addBlock', layout: 'lineBreak', index: 1 });
    const { dispatch } = renderCanvas(edited);

    fireEvent.keyDown(screen.getByRole('region', { name: 'Page canvas' }), { key: 'z', ctrlKey: true });
    fireEvent.keyDown(screen.getByRole('region', { name: 'Page canvas' }), {
      key: 'z',
      ctrlKey: true,
      shiftKey: true,
    });

    expect(dispatch).toHaveBeenCalledWith({ type: 'undo' });
    expect(dispatch).toHaveBeenCalledWith({ type: 'redo' });
  });
});

describe('the page layout controls', () => {
  it('are not on the canvas, which leaves page layout to the inspector', () => {
    renderCanvas(gridPage([rawBlock('html', { html: 'a' })]));
    expect(screen.queryByRole('radio', { name: 'Grid' })).toBeNull();
    expect(screen.queryByLabelText('Columns')).toBeNull();
  });
});

describe('the empty draft', () => {
  it('reports the missing page title', () => {
    const draft = { ...load([]), title: '' };
    renderCanvas(draft);
    expect(screen.getByRole('alert')).toHaveTextContent('The page has 1 issues.');
  });
});
