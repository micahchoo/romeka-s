/**
 * The page editor's draft model, exercised through its public functions. The
 * round-trip cases freeze the one-way rule in `docs/compat.md`: a key this
 * model does not know, a layout it does not register, and every attachment must
 * survive an edit and a save.
 */
import { describe, expect, it } from 'vitest';
import {
  draftReducer,
  emptyDraft,
  isDirty,
  nextBlockKey,
  toApi,
  toPageWrite,
  toSavedTemplate,
  validate,
} from '../src/pages/sites/editor/draft';
import type { ValidationIssue } from '../src/pages/sites/editor/draft';
import type { TemplateBlock, TemplateContent } from '../src/pages/sites/templates';
import {
  apply,
  at,
  attachment,
  blockAt,
  dataOf,
  find,
  keyOf,
  layouts,
  load,
  parsePage,
  rawBlock,
  spanOf,
} from './editor-draft-helpers';

describe('the page draft round trip', () => {
  const page = load(
    [
      rawBlock(
        'html',
        { html: '<p>Hi</p>', romeka: { preset: 'x' }, future: [1, { deep: true }] },
        {
          'o:layout_data': { class: 'hero', romeka_flag: 'keep' },
          'o:attachment': [
            {
              'o:item': { '@id': 'http://localhost/api/items/5', 'o:id': 5 },
              'o:media': { '@id': 'http://localhost/api/media/9', 'o:id': 9 },
              'o:caption': 'A caption',
            },
          ],
        },
      ),
      rawBlock('blockGroup', { span: '2', tag: 'future' }, { 'o:layout_data': { class: 'row' } }),
      rawBlock('lineBreak', { break_type: 'opaque' }),
      rawBlock('myModuleLayout', { anything: [1, 'two'], nested: { ok: true } }, {
        'o:layout_data': { custom: 'x', nested: { keep: true } },
      }),
      rawBlock('pageTitle', []),
    ],
    {
      'o:title': 'A page',
      'o:slug': 'a-page',
      'o:is_public': false,
      'o:layout': 'grid',
      'o:layout_data': { grid_columns: '3', romeka_page: { keep: 1 } },
    },
  );

  it('keeps the page members and the grid layout', () => {
    expect(page.title).toBe('A page');
    expect(page.slug).toBe('a-page');
    expect(page.isPublic).toBe(false);
    expect(page.layout).toBe('grid');
    expect(page.layoutData).toEqual({ grid_columns: '3', romeka_page: { keep: 1 } });
  });

  it('writes every block member back, unknown keys included', () => {
    const writes = toApi(page);
    expect(writes).toHaveLength(5);
    expect(at(writes, 0)).toEqual({
      layout: 'html',
      data: { html: '<p>Hi</p>', romeka: { preset: 'x' }, future: [1, { deep: true }] },
      layoutData: { class: 'hero', romeka_flag: 'keep' },
      attachments: [{ item: 5, media: 9, caption: 'A caption' }],
    });
    expect(at(writes, 1)).toEqual({
      layout: 'blockGroup',
      data: { span: '2', tag: 'future' },
      layoutData: { class: 'row' },
      attachments: [],
    });
    expect(at(writes, 3)).toEqual({
      layout: 'myModuleLayout',
      data: { anything: [1, 'two'], nested: { ok: true } },
      layoutData: { custom: 'x', nested: { keep: true } },
      attachments: [],
    });
    // pageTitle stores the empty PHP array.
    expect(at(writes, 4).data).toEqual([]);
  });

  it('writes the page header the adapter reads', () => {
    const site = parsePage([]).site;
    expect(toPageWrite(page, site.id)).toEqual({
      site: site.id,
      title: 'A page',
      slug: 'a-page',
      isPublic: false,
      layout: 'grid',
      layoutData: { grid_columns: '3', romeka_page: { keep: 1 } },
      blocks: toApi(page),
    });
  });

  it('reads a normal page back as the normal layout', () => {
    expect(load([rawBlock('html', { html: 'x' })]).layout).toBe('normal');
  });

  it('keeps the group span it was given when the following blocks allow it', () => {
    expect(spanOf(find(page, 'blockGroup'))).toBe(2);
    expect(layouts(page)).toEqual([
      'html',
      'blockGroup',
      'lineBreak',
      'myModuleLayout',
      'pageTitle',
    ]);
  });

  it('clamps a span larger than the blocks before the next group', () => {
    const clamped = load([
      rawBlock('blockGroup', { span: '9' }),
      rawBlock('html', { html: 'a' }),
      rawBlock('blockGroup', { span: '0' }),
    ]);
    expect(spanOf(blockAt(clamped.blocks, 0))).toBe(1);
  });

  it('keeps a block after a group outside it', () => {
    const outside = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'inside' }),
      rawBlock('html', { html: 'outside' }),
    ]);
    expect(spanOf(blockAt(outside.blocks, 0))).toBe(1);
    expect(layouts(outside)).toEqual(['blockGroup', 'html', 'html']);
    expect(at(toApi(outside), 0).data).toEqual({ span: '1' });
  });

  it('keeps an unknown layout whose data is a list', () => {
    const list = load([rawBlock('myModuleLayout', [1, 'two'])]);
    expect(at(toApi(list), 0).data).toEqual([1, 'two']);
  });

  it('keeps the empty page layout-data array', () => {
    const site = parsePage([]).site;
    expect(toPageWrite(load([]), site.id).layoutData).toEqual([]);
  });
});

describe('field, layout data and attachments', () => {
  it('sets one data field and keeps the others', () => {
    const state = load([rawBlock('html', { html: 'a', romeka: { keep: true } })]);
    const next = draftReducer(state, {
      type: 'setField',
      blockKey: keyOf(state, 'html'),
      field: 'html',
      value: '<p>b</p>',
    });
    expect(dataOf(at(next.blocks, 0))).toEqual({
      html: '<p>b</p>',
      romeka: { keep: true },
    });
  });

  it('leaves the derived span of a group alone', () => {
    const state = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'a' }),
    ]);
    const next = draftReducer(state, {
      type: 'setField',
      blockKey: keyOf(state, 'blockGroup'),
      field: 'span',
      value: 9,
    });
    expect(next).toBe(state);
    expect(spanOf(at(next.blocks, 0))).toBe(1);
  });

  it('sets one layout data member and keeps the unknown ones', () => {
    const state = load([
      rawBlock('html', { html: 'a' }, { 'o:layout_data': { class: 'old', romeka: 1 } }),
    ]);
    const next = draftReducer(state, {
      type: 'setLayoutData',
      blockKey: keyOf(state, 'html'),
      key: 'grid_column_span',
      value: '6',
    });
    expect(at(next.blocks, 0).layoutData).toEqual({
      class: 'old',
      romeka: 1,
      grid_column_span: '6',
    });
  });

  it('starts layout data from an absent member', () => {
    const state = load([rawBlock('html', { html: 'a' })]);
    const next = draftReducer(state, {
      type: 'setLayoutData',
      blockKey: keyOf(state, 'html'),
      key: 'class',
      value: 'hero',
    });
    expect(at(next.blocks, 0).layoutData).toEqual({ class: 'hero' });
  });

  it('replaces the attachments', () => {
    const state = load([rawBlock('media', {})]);
    const next = draftReducer(state, {
      type: 'setAttachments',
      blockKey: keyOf(state, 'media'),
      attachments: [attachment(7, 8, 'caption')],
    });
    expect(at(next.blocks, 0).attachments).toEqual([attachment(7, 8, 'caption')]);
    expect(at(toApi(next), 0).attachments).toEqual([
      { item: 7, media: 8, caption: 'caption' },
    ]);
  });

  it('retargets a block at another layout and resets its data', () => {
    const state = load([
      rawBlock('html', { html: 'a' }, {
        'o:layout_data': { class: 'keep' },
        'o:attachment': [{ 'o:item': null, 'o:media': null, 'o:caption': 'keep' }],
      }),
    ]);
    const next = draftReducer(state, {
      type: 'setLayout',
      blockKey: keyOf(state, 'html'),
      layout: 'lineBreak',
    });
    expect(at(next.blocks, 0)).toMatchObject({
      layout: 'lineBreak',
      data: { break_type: 'transparent' },
      layoutData: { class: 'keep' },
      attachments: [{ item: null, media: null, caption: 'keep' }],
    });
  });

  it('promotes the members when a group changes to a plain layout', () => {
    const state = load([
      rawBlock('blockGroup', { span: '1' }),
      rawBlock('html', { html: 'a' }),
    ]);
    const next = draftReducer(state, {
      type: 'setLayout',
      blockKey: keyOf(state, 'blockGroup'),
      layout: 'html',
    });
    expect(layouts(next)).toEqual(['html', 'html']);
    expect(at(next.blocks, 0).data).toEqual({});
  });
});

describe('page members', () => {
  it('sets the title, slug and visibility', () => {
    const state = apply(
      emptyDraft(),
      { type: 'setPage', field: 'title', value: 'T' },
      { type: 'setPage', field: 'slug', value: 't' },
      { type: 'setPage', field: 'isPublic', value: false },
    );
    expect(state.title).toBe('T');
    expect(state.slug).toBe('t');
    expect(state.isPublic).toBe(false);
  });

  it('switches between the normal flow and the grid', () => {
    const grid = draftReducer(emptyDraft(), { type: 'setPageLayout', layout: 'grid' });
    const normal = draftReducer(grid, { type: 'setPageLayout', layout: 'normal' });
    const site = parsePage([]).site;
    expect(grid.layout).toBe('grid');
    expect(toPageWrite(grid, site.id).layout).toBe('grid');
    expect(toPageWrite(normal, site.id).layout).toBeNull();
  });

  it('sets one page layout data member and keeps the unknown ones', () => {
    const state = load([], { 'o:layout_data': { romeka_page: { keep: 1 } } });
    const next = draftReducer(state, {
      type: 'setPageLayoutData',
      key: 'grid_columns',
      value: 4,
    });
    expect(next.layoutData).toEqual({ romeka_page: { keep: 1 }, grid_columns: 4 });
  });
});

describe('templates', () => {
  it('inserts a template in order with fresh keys', () => {
    const state = load([rawBlock('html', { html: 'a' })]);
    const blocks: readonly TemplateBlock[] = [
      { layout: 'pageTitle', data: {}, layoutData: null },
      { layout: 'lineBreak', data: { break_type: 'opaque' }, layoutData: null },
    ];
    const next = draftReducer(state, { type: 'insertBlocks', index: 1, blocks });
    expect(layouts(next)).toEqual(['html', 'pageTitle', 'lineBreak']);
    const keys = next.blocks.map((block) => block.key);
    expect(new Set(keys).size).toBe(3);
    expect(at(next.blocks, 1).data).toEqual({});
    expect(at(next.blocks, 2).data).toEqual({ break_type: 'opaque' });
  });

  it('applies a template to the page layout, its layout data and its blocks', () => {
    const template: TemplateContent = {
      id: 'gallery',
      layout: 'grid',
      layoutData: { grid_columns: 3 },
      blocks: [
        { layout: 'media', data: {}, layoutData: { grid_column_span: '1' } },
        { layout: 'media', data: {}, layoutData: { grid_column_span: '1' } },
      ],
    };
    const next = draftReducer(emptyDraft(), { type: 'applyTemplate', template });
    expect(next.layout).toBe('grid');
    expect(next.layoutData).toEqual({ grid_columns: 3 });
    expect(layouts(next)).toEqual(['media', 'media']);
    expect(at(next.blocks, 0).layoutData).toEqual({ grid_column_span: '1' });
    // A template's blocks carry no attachments.
    expect(at(next.blocks, 0).attachments).toEqual([]);
  });

  it('stores a draft as a saved template without attachments or a plain hash', () => {
    const state = load([
      rawBlock(
        'html',
        { html: '<p>x</p>' },
        {
          'o:layout_data': {
            romeka: { block: 'hero', v: 1, settings: { heading: 'Hi' }, plain_hash: 'abc' },
          },
          'o:attachment': [
            { 'o:item': { '@id': 'http://localhost/api/items/5', 'o:id': 5 }, 'o:media': null, 'o:caption': 'c' },
          ],
        },
      ),
    ]);
    expect(toSavedTemplate(state, 't1', 'My template', 'A description')).toEqual({
      id: 't1',
      name: 'My template',
      description: 'A description',
      layout: 'normal',
      layoutData: [],
      blocks: [
        {
          layout: 'html',
          data: { html: '<p>x</p>' },
          layoutData: { romeka: { block: 'hero', v: 1, settings: { heading: 'Hi' } } },
        },
      ],
    });
  });
});

describe('undo and redo', () => {
  it('walks back and forward through the edits', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    const added = draftReducer(start, { type: 'addBlock', layout: 'lineBreak', index: 1 });
    const edited = draftReducer(added, {
      type: 'setField',
      blockKey: keyOf(added, 'lineBreak'),
      field: 'break_type',
      value: 'opaque',
    });

    const undone = draftReducer(edited, { type: 'undo' });
    expect(layouts(undone)).toEqual(['html', 'lineBreak']);
    expect(dataOf(find(undone, 'lineBreak'))).toEqual({ break_type: 'transparent' });

    const undoneTwice = draftReducer(undone, { type: 'undo' });
    expect(layouts(undoneTwice)).toEqual(['html']);

    const redone = draftReducer(undoneTwice, { type: 'redo' });
    expect(layouts(redone)).toEqual(['html', 'lineBreak']);
  });

  it('does nothing at either end of the history', () => {
    const state = emptyDraft();
    expect(draftReducer(state, { type: 'undo' })).toBe(state);
    expect(draftReducer(state, { type: 'redo' })).toBe(state);
  });

  it('drops the redo stack when a new edit arrives', () => {
    const start = emptyDraft();
    const added = draftReducer(start, { type: 'addBlock', layout: 'html', index: 0 });
    const undone = draftReducer(added, { type: 'undo' });
    const branched = draftReducer(undone, { type: 'addBlock', layout: 'lineBreak', index: 0 });
    expect(branched.future).toEqual([]);
  });

  it('coalesces a run of edits to one field into a single undo entry', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    const key = keyOf(start, 'html');
    const typed = apply(
      start,
      { type: 'setField', blockKey: key, field: 'html', value: 'ab' },
      { type: 'setField', blockKey: key, field: 'html', value: 'abc' },
      { type: 'setField', blockKey: key, field: 'html', value: 'abcd' },
    );

    expect(typed.past).toHaveLength(1);
    const undone = draftReducer(typed, { type: 'undo' });
    expect(dataOf(find(undone, 'html'))).toEqual({ html: 'a' });
    const redone = draftReducer(undone, { type: 'redo' });
    expect(dataOf(find(redone, 'html'))).toEqual({ html: 'abcd' });
  });

  it('starts a new undo entry when an edit of another field interrupts the run', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    const key = keyOf(start, 'html');
    const typed = apply(
      start,
      { type: 'setField', blockKey: key, field: 'html', value: 'ab' },
      { type: 'setLayoutData', blockKey: key, key: 'class', value: 'hero' },
      { type: 'setField', blockKey: key, field: 'html', value: 'abc' },
    );

    expect(typed.past).toHaveLength(3);
    const undone = draftReducer(typed, { type: 'undo' });
    expect(dataOf(find(undone, 'html'))).toEqual({ html: 'ab' });
  });

  it('exposes the key the next addBlock gives its block', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    const key = nextBlockKey(start);
    const added = draftReducer(start, { type: 'addBlock', layout: 'lineBreak', index: 1 });
    expect(at(added.blocks, 1).key).toBe(key);
  });

  it('tracks dirt against the saved snapshot', () => {
    const start = load([rawBlock('html', { html: 'a' })]);
    expect(isDirty(start)).toBe(false);

    const edited = draftReducer(start, {
      type: 'setField',
      blockKey: keyOf(start, 'html'),
      field: 'html',
      value: 'b',
    });
    expect(isDirty(edited)).toBe(true);
    expect(isDirty(draftReducer(edited, { type: 'undo' }))).toBe(false);

    const saved = draftReducer(edited, { type: 'markSaved' });
    expect(isDirty(saved)).toBe(false);
  });
});

describe('validate', () => {
  const issuesOf = (blocks: readonly unknown[]): readonly ValidationIssue[] => validate(load(blocks));

  it('reports a required field the block omits', () => {
    const state = load([rawBlock('oembed', {})]);
    expect(validate(state)).toContainEqual({
      kind: 'required',
      blockKey: at(state.blocks, 0).key,
      field: 'url',
    });
  });

  it('accepts a required field once it has a value', () => {
    expect(issuesOf([rawBlock('oembed', { url: 'https://example.org' })])).toEqual([]);
  });

  it('reports an integer below the schema minimum', () => {
    const state = load([rawBlock('tableOfContents', { depth: 0 })]);
    expect(validate(state)).toContainEqual({
      kind: 'min',
      blockKey: at(state.blocks, 0).key,
      field: 'depth',
      min: 1,
    });
  });

  it('reports an integer below a zero minimum', () => {
    const state = load([rawBlock('listOfSites', { limit: -1 })]);
    expect(validate(state)).toContainEqual({
      kind: 'min',
      blockKey: at(state.blocks, 0).key,
      field: 'limit',
      min: 0,
    });
  });

  it('reports a value that is not a whole number', () => {
    const state = load([rawBlock('browsePreview', { limit: 'twelve' })]);
    expect(validate(state)).toContainEqual({
      kind: 'type',
      blockKey: at(state.blocks, 0).key,
      field: 'limit',
    });
  });

  it('ignores an unknown layout and an unknown data key', () => {
    expect(
      issuesOf([rawBlock('myModuleLayout', { any: 'thing' }), rawBlock('html', { html: 'x' })]),
    ).toEqual([]);
  });

  it('reports an empty page title', () => {
    expect(validate(emptyDraft())).toContainEqual({
      kind: 'required',
      blockKey: null,
      field: 'title',
    });
  });
});
