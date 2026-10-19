/**
 * The preview frame modules against a real DOM: the marker map, the guard on
 * the frame document, the insert slots and the scroll anchor.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BlockKey } from '../src/pages/sites/editor/draft';
import { restoreAnchor, takeAnchor } from '../src/pages/sites/editor/frame/anchor';
import { insertSlots } from '../src/pages/sites/editor/frame/geometry';
import { guardFrame } from '../src/pages/sites/editor/frame/guard';
import { readFrame } from '../src/pages/sites/editor/frame/markers';
import { at, load, rawBlock } from './editor-draft-helpers';

/** A page of an html block, a group with one member, and a line break. */
const draft = load([
  rawBlock('html', { html: '<p>One</p>' }),
  rawBlock('blockGroup', { span: '1' }),
  rawBlock('html', { html: '<p>Member</p>' }),
  rawBlock('lineBreak', { break_type: 'opaque' }),
]);
const order = draft.blocks.map((block) => block.key);
const [html, group, member, lineBreak] = order;
const groups: ReadonlySet<BlockKey> = new Set(group === undefined ? [] : [group]);

/** The preview render of `draft`, with a spoofed marker inside the first block's stored HTML. */
const RENDER = `
  <div class="blocks-inner">
    <div class="block block-html" data-romeka-block="1">
      <p id="one">One <span data-romeka-block="4" id="spoof">spoof</span></p>
      <a id="link" href="https://example.org/">away</a>
    </div>
    <div class="block block-blockGroup" data-romeka-block="2" id="group">
      <div class="block block-html" data-romeka-block="3"><p id="member">Member</p></div>
    </div>
    <div class="block block-lineBreak" data-romeka-block="4"></div>
    <div class="block" data-romeka-block="9" id="stray"></div>
  </div>`;

const mountRender = (): Document => {
  document.body.innerHTML = RENDER;
  return document;
};

const byId = (id: string): Element => {
  const element = document.getElementById(id);
  if (element === null) {
    throw new Error(`no #${id}`);
  }
  return element;
};

/** Gives an element a fixed rect; jsdom has no layout. */
const placeAt = (element: Element, top: number, height: number, left = 0, width = 600): void => {
  Object.defineProperty(element, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ top, left, width, height, bottom: top + height, right: left + width, x: left, y: top }),
  });
};

afterEach(() => {
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

describe('readFrame', () => {
  it('maps every real marker to the key the request sent at that position', () => {
    const map = readFrame(mountRender(), order, groups);
    expect([...map.elements.keys()]).toEqual([html, group, member, lineBreak]);
  });

  it('resolves a node to the innermost real block above it', () => {
    const map = readFrame(mountRender(), order, groups);
    expect(map.blockAt(byId('member'))).toBe(member);
    expect(map.blockAt(byId('group'))).toBe(group);
    expect(map.blockAt(byId('one').firstChild ?? byId('one'))).toBe(html);
  });

  it('ignores a marker inside a block and a marker no request position names', () => {
    const map = readFrame(mountRender(), order, groups);
    expect(map.blockAt(byId('spoof'))).toBe(html);
    expect(map.blockAt(byId('stray'))).toBeNull();
    expect([...map.elements.values()]).not.toContain(byId('spoof'));
  });
});

describe('guardFrame', () => {
  it('stops a link from navigating and reports what was clicked until detached', () => {
    const doc = mountRender();
    const picked: Node[] = [];
    const detach = guardFrame(doc, { onPoint: () => {}, onPick: (node) => picked.push(node) });

    const click = new MouseEvent('click', { bubbles: true, cancelable: true });
    byId('link').dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    expect(picked).toEqual([byId('link')]);

    detach();
    const after = new MouseEvent('click', { bubbles: true, cancelable: true });
    byId('link').dispatchEvent(after);
    expect(after.defaultPrevented).toBe(false);
    expect(picked).toHaveLength(1);
  });

  it('stops a form submit and reports hover moves', () => {
    const doc = mountRender();
    const pointed: (Node | null)[] = [];
    const detach = guardFrame(doc, { onPoint: (node) => pointed.push(node), onPick: () => {} });
    const form = doc.createElement('form');
    doc.body.append(form);
    const submit = new Event('submit', { bubbles: true, cancelable: true });
    form.dispatchEvent(submit);
    expect(submit.defaultPrevented).toBe(true);

    byId('member').dispatchEvent(new Event('pointermove', { bubbles: true }));
    doc.documentElement.dispatchEvent(new Event('pointerleave'));
    expect(pointed).toEqual([byId('member'), null]);
    detach();
  });
});

describe('insertSlots', () => {
  const frame = document.createElement('iframe');

  it('offers a slot before each top-level node, one into each group and one at the end', () => {
    const map = readFrame(mountRender(), order, groups);
    const slots = insertSlots(draft, map, frame);
    expect(slots.map((slot) => slot.target)).toEqual([
      { kind: 'top', index: 0 },
      { kind: 'top', index: 1 },
      { kind: 'group', group, index: 3 },
      { kind: 'top', index: 3 },
      { kind: 'top', index: 4 },
    ]);
    expect(slots.every((slot) => slot.shape === 'line')).toBe(true);
  });

  it('puts a line slot halfway across the gap between two blocks', () => {
    const map = readFrame(mountRender(), order, groups);
    placeAt(byId('group').previousElementSibling ?? byId('group'), 0, 100);
    placeAt(byId('group'), 120, 200);
    const slots = insertSlots(draft, map, frame);
    expect(at(slots, 1).box.top).toBe(110);
    expect(at(slots, 2).box.top).toBe(320);
  });

  it('gives a block missing from the render no slot of its own', () => {
    const map = readFrame(mountRender(), order.slice(0, 1), groups);
    expect(insertSlots(draft, map, frame).map((slot) => slot.target)).toEqual([
      { kind: 'top', index: 0 },
      { kind: 'top', index: 4 },
    ]);
  });

  it('uses corner chips on a grid page', () => {
    const grid = { ...draft, layout: 'grid' as const };
    const map = readFrame(mountRender(), order, groups);
    placeAt(byId('group'), 50, 80, 300, 200);
    const slots = insertSlots(grid, map, frame);
    const before = slots.find((slot) => slot.target.kind === 'top' && slot.target.index === 1);
    expect(before?.shape).toBe('chip');
    expect(before?.box).toEqual({ top: 50, left: 300, width: 0, height: 0 });
  });
});

describe('the scroll anchor', () => {
  it('keeps the selected block at the same distance from the viewport top after a re-render', () => {
    mountRender();
    const first = readFrame(document, order, groups);
    placeAt(byId('group'), 40, 300);
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 500 });
    const anchor = takeAnchor(window, first, group ?? null);
    expect(anchor).toEqual({ key: group, offset: 40, scrollY: 500 });

    // The new document starts at the top, and a block above grew by 100 px.
    mountRender();
    const second = readFrame(document, order, groups);
    placeAt(byId('group'), 640, 300);
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 0 });
    const scrollTo = vi.spyOn(window, 'scrollTo').mockImplementation(() => {});
    restoreAnchor(window, second, anchor);
    expect(scrollTo).toHaveBeenCalledWith(0, 600);
  });
});
