/**
 * The structural edits: adding, removing and moving blocks, and the group span
 * that has to follow them. The property case drives random sequences and checks
 * the invariant after every step.
 */
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import {
  dataObject,
  draftReducer,
  emptyDraft,
  toApi,
} from '../src/pages/sites/editor/draft';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import {
  at,
  blockAt,
  keyOf,
  layouts,
  load,
  rawBlock,
  seeded,
  spanOf,
} from './editor-draft-helpers';

describe('addBlock', () => {
  it('starts a known layout from the schema defaults', () => {
    const state = draftReducer(emptyDraft(), {
      type: 'addBlock',
      layout: 'lineBreak',
      index: 0,
    });
    expect(at(state.blocks, 0)).toMatchObject({
      layout: 'lineBreak',
      data: { break_type: 'transparent' },
      layoutData: null,
      attachments: [],
    });
  });

  it('starts a block group at span zero', () => {
    const state = draftReducer(emptyDraft(), {
      type: 'addBlock',
      layout: 'blockGroup',
      index: 0,
    });
    expect(dataObject(at(state.blocks, 0))).toEqual({ span: 0 });
  });

  it('starts a layout with no defaults (and an unknown layout) empty', () => {
    const state = draftReducer(
      draftReducer(emptyDraft(), { type: 'addBlock', layout: 'html', index: 0 }),
      { type: 'addBlock', layout: 'myModuleLayout', index: 1 },
    );
    expect(at(state.blocks, 0).data).toEqual({});
    expect(at(state.blocks, 1).data).toEqual({});
  });

  it('inserts at the index it is given', () => {
    const state = draftReducer(load([rawBlock('html', { html: 'a' })]), {
      type: 'addBlock',
      layout: 'pageTitle',
      index: 0,
    });
    expect(layouts(state)).toEqual(['pageTitle', 'html']);
  });

  it('appends after a trailing group instead of into it', () => {
    const state = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'inside' }),
    ]);
    const next = draftReducer(state, {
      type: 'addBlock',
      layout: 'lineBreak',
      index: 2,
    });
    expect(layouts(next)).toEqual(['blockGroup', 'html', 'lineBreak']);
    expect(spanOf(blockAt(next.blocks, 0))).toBe(1);
  });

  it('appends into a named group, including an empty one', () => {
    const state = load([
      rawBlock('blockGroup', { span: '0' }),
      rawBlock('html', { html: 'outside' }),
    ]);
    const next = draftReducer(state, {
      type: 'addBlock',
      layout: 'lineBreak',
      index: 0,
      into: keyOf(state, 'blockGroup'),
    });
    expect(layouts(next)).toEqual(['blockGroup', 'lineBreak', 'html']);
    expect(spanOf(blockAt(next.blocks, 0))).toBe(1);
  });
});

describe('removeBlock', () => {
  it('removes a member and shrinks its group', () => {
    const state = load([
      rawBlock('blockGroup', { span: '2' }),
      rawBlock('html', { html: 'a' }),
      rawBlock('lineBreak', {}),
      rawBlock('pageTitle', []),
    ]);
    const removed = draftReducer(state, {
      type: 'removeBlock',
      blockKey: keyOf(state, 'lineBreak'),
    });
    expect(layouts(removed)).toEqual(['blockGroup', 'html', 'pageTitle']);
    expect(spanOf(blockAt(removed.blocks, 0))).toBe(1);
  });

  it('removes only the group marker and promotes its members', () => {
    const state = load([
      rawBlock('blockGroup', { span: '2' }),
      rawBlock('html', { html: 'a' }),
      rawBlock('lineBreak', {}),
      rawBlock('pageTitle', []),
    ]);
    const removed = draftReducer(state, {
      type: 'removeBlock',
      blockKey: keyOf(state, 'blockGroup'),
    });
    expect(layouts(removed)).toEqual(['html', 'lineBreak', 'pageTitle']);
  });
});

describe('moveBlock', () => {
  const groupPage = (): PageDraft =>
    load([
      rawBlock('pageTitle', []),
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'inside' }),
      rawBlock('lineBreak', {}),
    ]);

  it('moves a block into a group and grows the span', () => {
    const state = groupPage();
    const moved = draftReducer(state, {
      type: 'moveBlock',
      blockKey: keyOf(state, 'lineBreak'),
      toIndex: 2,
    });
    expect(layouts(moved)).toEqual(['pageTitle', 'blockGroup', 'lineBreak', 'html']);
    expect(spanOf(blockAt(moved.blocks, 1))).toBe(2);
  });

  it('moves a member out of a group and shrinks the span', () => {
    const state = load([
      rawBlock('pageTitle', []),
      rawBlock('blockGroup', { span: '2' }),
      rawBlock('html', { html: 'a' }),
      rawBlock('lineBreak', {}),
    ]);
    const moved = draftReducer(state, {
      type: 'moveBlock',
      blockKey: keyOf(state, 'lineBreak'),
      toIndex: 3,
    });
    expect(layouts(moved)).toEqual(['pageTitle', 'blockGroup', 'html', 'lineBreak']);
    expect(spanOf(blockAt(moved.blocks, 1))).toBe(1);
  });

  it('moves only the group marker and leaves its members behind', () => {
    const state = groupPage();
    const moved = draftReducer(state, {
      type: 'moveBlock',
      blockKey: keyOf(state, 'blockGroup'),
      toIndex: 3,
    });
    expect(layouts(moved)).toEqual(['pageTitle', 'html', 'lineBreak', 'blockGroup']);
    expect(spanOf(blockAt(moved.blocks, 3))).toBe(0);
  });

  it('clamps an index past either end', () => {
    const state = groupPage();
    const moved = draftReducer(state, {
      type: 'moveBlock',
      blockKey: keyOf(state, 'pageTitle'),
      toIndex: 99,
    });
    expect(layouts(moved)).toEqual(['blockGroup', 'html', 'lineBreak', 'pageTitle']);
  });

  it('moves a block into a named empty group', () => {
    const state = load([
      rawBlock('blockGroup', { span: '0' }),
      rawBlock('html', { html: 'a' }),
      rawBlock('lineBreak', {}),
    ]);
    const moved = draftReducer(state, {
      type: 'moveBlock',
      blockKey: keyOf(state, 'lineBreak'),
      toIndex: 0,
      into: keyOf(state, 'blockGroup'),
    });
    expect(layouts(moved)).toEqual(['blockGroup', 'lineBreak', 'html']);
    expect(spanOf(blockAt(moved.blocks, 0))).toBe(1);
  });
});

describe('the group span invariant under random edits', () => {
  const base = (): readonly unknown[] => [
    rawBlock('pageTitle', []),
    rawBlock('blockGroup', { span: '2' }),
    rawBlock('html', { html: 'a' }),
    rawBlock('media', {}),
    rawBlock('lineBreak', {}),
    rawBlock('blockGroup', { span: '1' }),
    rawBlock('html', { html: 'b' }),
    rawBlock('pageTitle', []),
  ];

  const checkSpans = (state: PageDraft): void => {
    state.blocks.forEach((block, index) => {
      if (block.layout === 'blockGroup') {
        const span = spanOf(block);
        expect(span).toBeGreaterThanOrEqual(0);
        let run = 0;
        for (let next = index + 1; next < state.blocks.length; next += 1) {
          if (blockAt(state.blocks, next).layout === 'blockGroup') {
            break;
          }
          run += 1;
        }
        expect(span).toBeLessThanOrEqual(run);
        expect(dataObject(block).span).toBe(span);
      }
    });
    for (const write of toApi(state)) {
      if (write.layout === 'blockGroup') {
        const data = isRecord(write.data) ? write.data : {};
        expect(typeof data.span).toBe('string');
      }
    }
  };

  it('holds after every action in a seeded sequence', () => {
    const random = seeded(20261018);
    const addable = ['html', 'lineBreak', 'blockGroup', 'media', 'pageTitle', 'myModuleLayout'];
    let state = load(base());
    checkSpans(state);

    for (let step = 0; step < 250; step += 1) {
      const choice = random();
      if (choice < 0.2) {
        const layout = at(addable, Math.floor(random() * addable.length));
        const index = Math.floor(random() * (state.blocks.length + 1));
        state = draftReducer(state, { type: 'addBlock', layout, index });
      } else if (choice < 0.4 && state.blocks.length > 0) {
        const block = at(state.blocks, Math.floor(random() * state.blocks.length));
        state = draftReducer(state, { type: 'removeBlock', blockKey: block.key });
      } else if (choice < 0.7 && state.blocks.length > 0) {
        const block = at(state.blocks, Math.floor(random() * state.blocks.length));
        const toIndex = Math.floor(random() * state.blocks.length);
        state = draftReducer(state, { type: 'moveBlock', blockKey: block.key, toIndex });
      } else if (choice < 0.8) {
        state = draftReducer(state, {
          type: 'insertBlocks',
          index: Math.floor(random() * (state.blocks.length + 1)),
          blocks: [
            {
              layout: 'lineBreak',
              data: { break_type: 'opaque' },
              layoutData: null,
            },
          ],
        });
      } else if (choice < 0.9 && state.blocks.length > 0) {
        const block = at(state.blocks, Math.floor(random() * state.blocks.length));
        state = draftReducer(state, {
          type: 'setLayout',
          blockKey: block.key,
          layout: block.layout === 'blockGroup' ? 'html' : 'blockGroup',
        });
      } else {
        state = draftReducer(state, { type: 'undo' });
      }
      checkSpans(state);
    }
  });
});
