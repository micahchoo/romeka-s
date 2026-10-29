/**
 * Whether a block is written (`editor/written.ts`): the count the block list
 * and the site hub show, the blank-HTML rule the server shares, and the
 * template text an older page still holds, which the editor offers to clear.
 */
import { describe, expect, it } from 'vitest';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { BUILTIN_SECTIONS, BUILTIN_TEMPLATES } from '../src/pages/sites/templateRegistry';
import { settingsOf } from '../src/pages/sites/editor/romeka';
import {
  FORMER_TEXT,
  clearTemplateText,
  foldSpace,
  previewHints,
  storedUnwrittenCount,
  templateTextKeys,
  unwrittenCount,
  unwrittenField,
} from '../src/pages/sites/editor/written';
import { isBlankHtml } from '../src/pages/sites/editor/blankHtml';
import { readLayoutData } from '../src/pages/sites/editor/values';
import { apply, at, blockAt, dataOf, load, parsePage, rawBlock } from './editor-draft-helpers';

const template = (id: string) => {
  const found = BUILTIN_TEMPLATES.find((entry) => entry.id === id);
  if (found === undefined) {
    throw new Error(`expected the ${id} template`);
  }
  return found;
};

const contentOf = (blocks: typeof BUILTIN_TEMPLATES[number]['blocks']) =>
  blocks.map((block) => ({ ...block, layoutData: readLayoutData(block.layoutData) }));

const hero = (settings: Record<string, string>) =>
  rawBlock('html', { html: '' }, {
    'o:layout_data': { romeka: { block: 'hero', v: 2, settings } },
  });

describe('the blank-HTML rule', () => {
  it('agrees with every sample the server ships', () => {
    expect(BLOCK_REGISTRY.blankHtml.length).toBeGreaterThan(5);
    for (const sample of BLOCK_REGISTRY.blankHtml) {
      expect(isBlankHtml(sample.html), sample.html).toBe(sample.blank);
    }
  });
});

describe('the not-written count', () => {
  it('counts the blocks a template leaves for the curator to write', () => {
    // The hero's heading, the introduction and the list of pages are empty;
    // the item preview is written.
    expect(unwrittenCount(BLOCK_REGISTRY, contentOf(template('exhibit-landing').blocks))).toBe(3);
    expect(unwrittenCount(BLOCK_REGISTRY, contentOf(template('about').blocks))).toBe(1);
    expect(unwrittenCount(BLOCK_REGISTRY, contentOf(template('gallery').blocks))).toBe(0);
  });

  it('counts each empty card of a section, and not the group', () => {
    const cards = BUILTIN_SECTIONS.find((section) => section.id === 'three-cards');
    expect(cards).toBeDefined();
    expect(unwrittenCount(BLOCK_REGISTRY, contentOf(cards?.blocks ?? []))).toBe(3);
  });

  it('names the field a block needs, and stops counting once it is written', () => {
    const page = parsePage([
      hero({ heading: '' }),
      rawBlock('html', { html: '<p><br></p>' }),
      rawBlock('html', { html: '<p>Written</p>' }),
      rawBlock('listOfPages', { pagelist: '[]' }),
      rawBlock('pageTitle', {}),
    ]);
    expect(storedUnwrittenCount(page.blocks)).toBe(3);
    const state = load([hero({ heading: '' })]);
    expect(unwrittenField(BLOCK_REGISTRY, blockAt(state.blocks, 0))?.key).toBe('heading');
    const typed = apply(state, {
      type: 'setLayoutDataValues',
      blockKey: blockAt(state.blocks, 0).key,
      values: { 'romeka.heading': 'Harbour letters' },
    });
    expect(unwrittenField(BLOCK_REGISTRY, blockAt(typed.blocks, 0))).toBeNull();
  });
});

describe('former template text', () => {
  it('matches a stored value after whitespace folds', () => {
    expect(FORMER_TEXT.has('Your exhibit title')).toBe(true);
    expect(foldSpace('  Your   exhibit\n title ')).toBe('Your exhibit title');
    const state = load([
      hero({ heading: 'Your  exhibit title', subheading: 'Our own words' }),
      rawBlock('html', {
        html: '<p>About this site. Replace this text with the story behind the collection.</p>',
      }),
      rawBlock('html', { html: '<p>About this site.</p>' }),
    ]);
    expect(templateTextKeys(BLOCK_REGISTRY, blockAt(state.blocks, 0))).toEqual(['heading']);
    expect(templateTextKeys(BLOCK_REGISTRY, blockAt(state.blocks, 1))).toEqual(['html']);
    expect(templateTextKeys(BLOCK_REGISTRY, blockAt(state.blocks, 2))).toEqual([]);
  });

  it('clears it in one undoable edit and keeps the words the curator wrote', () => {
    const state = load([hero({ heading: 'Your exhibit title', subheading: 'Our own words' })]);
    const block = blockAt(state.blocks, 0);
    const action = clearTemplateText(BLOCK_REGISTRY, block);
    expect(action).not.toBeNull();
    const cleared = action === null ? state : apply(state, action);
    expect(settingsOf(blockAt(cleared.blocks, 0))).toMatchObject({
      heading: '',
      subheading: 'Our own words',
    });
    expect(unwrittenField(BLOCK_REGISTRY, blockAt(cleared.blocks, 0))?.key).toBe('heading');
    const undone = apply(cleared, { type: 'undo' });
    expect(settingsOf(blockAt(undone.blocks, 0)).heading).toBe('Your exhibit title');
  });

  it('clears an upstream block in its own data', () => {
    const state = load([rawBlock('html', { html: '<p>A line worth remembering.</p>' })]);
    const action = clearTemplateText(BLOCK_REGISTRY, blockAt(state.blocks, 0));
    const cleared = action === null ? state : apply(state, action);
    expect(dataOf(blockAt(cleared.blocks, 0)).html).toBe('');
  });
});

describe('the preview hints', () => {
  it('carry the text of every primary field in the active locale', () => {
    const hints = previewHints(BLOCK_REGISTRY, (key) => `[${key}]`);
    expect(hints).toMatchObject({
      'fieldExample.hero.heading': '[fieldExample.hero.heading]',
      'fieldExample.html.html': '[fieldExample.html.html]',
      'fieldExample.quote.text': '[fieldExample.quote.text]',
      'fieldExample.listOfPages.pagelist': '[fieldExample.listOfPages.pagelist]',
    });
    expect(Object.keys(hints)).toHaveLength(4);
    expect(at(Object.values(hints), 0)).toMatch(/^\[/);
  });
});
