/**
 * Sections in the page editor: the Section panel a group shows, the slots the
 * Outline draws and a drag into one, the section templates in the block
 * picker, and the shared styling (anchor, theme colours) the styling form
 * writes. Each case drives a control and reads the draft the reducer keeps,
 * so it breaks when a gesture stops reaching the stored layout data.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { useReducer, useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import type { JsonValue } from '../src/api/types';
import { PageCanvas } from '../src/pages/sites/editor/canvas';
import type { BlockDraft, BlockKey, PageDraft } from '../src/pages/sites/editor/draft';
import { validate } from '../src/pages/sites/editor/draft';
import { readPath, writePaths } from '../src/pages/sites/editor/layoutPaths';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { Inspector } from '../src/pages/sites/editor/shell/Inspector';
import { layoutDataObject } from '../src/pages/sites/editor/values';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { SECTION_SHAPES } from '../src/pages/sites/editor/sections';
import { blockAt, load, rawBlock, spanOf } from './editor-draft-helpers';
import { SITE_ID } from './editor-fields-helpers';

const OPTIONS = { pages: [], itemSets: [], classes: [] };

const stored = (block: BlockDraft, path: string): JsonValue | undefined =>
  readPath(layoutDataObject(block), path);

/** A group of two HTML blocks with `romeka` as its layout data, and one block after it. */
const sectionPage = (romeka: JsonValue | null): PageDraft =>
  load([
    rawBlock('blockGroup', { span: 2 }, {
      'o:layout_data':
        romeka === null ? null : { romeka: { block: 'blockGroup', v: 2, settings: romeka } },
    }),
    rawBlock('html', { html: '<p>Left</p>' }),
    rawBlock('html', { html: '<p>Right</p>' }),
    rawBlock('html', { html: '<p>Outside</p>' }),
  ]);

/** The Outline and the inspector over one reducer, as the editor shell wires them. */
const Editor = ({
  initial,
  onDraft,
}: {
  readonly initial: PageDraft;
  readonly onDraft: (draft: PageDraft) => void;
}): ReactElement => {
  const [draft, dispatch] = useReducer(draftReducer, initial);
  const [selected, setSelected] = useState<BlockKey | null>(null);
  onDraft(draft);
  const block = draft.blocks.find((entry) => entry.key === selected) ?? null;
  return (
    <>
      <PageCanvas
        draft={draft}
        siteSettings={null}
        selection={selected === null ? null : { kind: 'block', key: selected, from: 'outline' }}
        onSelect={setSelected}
        dispatch={dispatch}
      />
      {block === null ? null : (
        <section aria-label="Inspector">
          <Inspector
            key={block.key}
            draft={draft}
            block={block}
            dispatch={dispatch}
            issues={validate(draft)}
            siteId={SITE_ID}
            options={OPTIONS}
            onDeselect={() => {
              setSelected(null);
            }}
            onSelect={setSelected}
          />
        </section>
      )}
    </>
  );
};

const renderEditor = (initial: PageDraft): { readonly current: () => PageDraft } => {
  let latest = initial;
  render(
    <Editor
      initial={initial}
      onDraft={(draft) => {
        latest = draft;
      }}
    />,
  );
  return { current: () => latest };
};

const card = (key: string): HTMLElement => {
  const element = document.querySelector(`li.canvas-card[data-key="${key}"]`);
  if (!(element instanceof HTMLElement)) {
    throw new Error(`no card for ${key}`);
  }
  return element;
};

const inspector = (): HTMLElement => screen.getByRole('region', { name: 'Inspector' });

describe('the Section panel', () => {
  it('sets the layout from a diagram and shows the group blocks as slots', async () => {
    const initial = sectionPage(null);
    const group = blockAt(initial.blocks, 0);
    const editor = renderEditor(initial);
    fireEvent.click(card(group.key));

    const choices = await within(inspector()).findByRole('radiogroup', { name: 'Arrangement' });
    const options = within(choices).getAllByRole('radio');
    expect(options.map((option) => option.textContent)).toEqual([
      'Stacked',
      'Two halves',
      'Narrow and wide',
      'Wide and narrow',
      'Two cards',
      'Three cards',
      'Four cards',
      'Full-width band',
    ]);
    expect(within(inspector()).getByText(/No layout yet/)).toBeInTheDocument();

    fireEvent.click(within(choices).getByRole('radio', { name: 'Narrow and wide' }));

    const saved = blockAt(editor.current().blocks, 0);
    expect(stored(saved, 'romeka.section.layout')).toBe('split-40-60');
    expect(within(choices).getByRole('radio', { name: 'Narrow and wide' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(within(inspector()).getByRole('button', { name: /Slot 1/ })).toHaveTextContent('HTML');
    expect(within(inspector()).getByRole('button', { name: /Slot 2/ })).toHaveTextContent('HTML');
    expect(within(inspector()).queryByText(/No layout yet/)).toBeNull();
  });

  it('writes the alignment and gap under the section, and a slot selects its block', async () => {
    const initial = sectionPage({ section: { layout: 'cards-3' } });
    const group = blockAt(initial.blocks, 0);
    const second = blockAt(initial.blocks, 2);
    const editor = renderEditor(initial);
    fireEvent.click(card(group.key));

    fireEvent.change(await within(inspector()).findByLabelText('Vertical alignment'), {
      target: { value: 'center' },
    });
    fireEvent.change(within(inspector()).getByLabelText('Space between blocks'), {
      target: { value: 'large' },
    });
    const saved = blockAt(editor.current().blocks, 0);
    expect(stored(saved, 'romeka.section')).toEqual({
      layout: 'cards-3',
      align: 'center',
      gap: 'large',
    });
    // Three cards a row, two blocks: the third slot waits for a block.
    expect(within(inspector()).getByText(/Slot 3 is empty/)).toBeInTheDocument();

    fireEvent.click(within(inspector()).getByRole('button', { name: /Slot 2/ }));
    expect(card(second.key)).toHaveAttribute('aria-current', 'true');
  });

  it('keeps the background with the section and out of the block styling', async () => {
    const initial = sectionPage({ section: { layout: 'band' } });
    renderEditor(initial);
    fireEvent.click(card(blockAt(initial.blocks, 0).key));
    const panel = await within(inspector()).findByRole('group', { name: 'Section' });
    expect(within(panel).getByRole('group', { name: 'Background' })).toBeInTheDocument();
    const styling = within(inspector()).getByRole('group', { name: 'Block styling' });
    expect(within(styling).queryByRole('group', { name: 'Background' })).toBeNull();
  });
});

describe('the Outline draws a section as its slots', () => {
  it('lays the slots out in the section columns', () => {
    const initial = sectionPage({ section: { layout: 'split-40-60' } });
    renderEditor(initial);
    const slots = document.querySelector('.canvas-slots');
    expect(slots).toBeInstanceOf(HTMLElement);
    expect(slots instanceof HTMLElement ? slots.style.gridTemplateColumns : '').toBe(
      'minmax(0, 2fr) minmax(0, 3fr)',
    );
    expect(screen.getByRole('listitem', { name: 'Section, Narrow and wide' })).toBeInTheDocument();
  });

  it('drops a dragged block into the slot it lands on', () => {
    const initial = sectionPage({ section: { layout: 'split-50-50' } });
    const [group, left, right, outside] = [0, 1, 2, 3].map((index) =>
      blockAt(initial.blocks, index),
    );
    if (group === undefined || left === undefined || right === undefined || outside === undefined) {
      throw new Error('four blocks');
    }
    const editor = renderEditor(initial);
    const transfer = { setData: () => undefined, effectAllowed: 'move' };

    fireEvent.dragStart(card(outside.key), { dataTransfer: transfer });
    const second = document.querySelector('.canvas-slot[data-slot="2"]');
    fireEvent.drop(second ?? document.body, { dataTransfer: transfer });

    const after = editor.current().blocks;
    expect(after.map((block) => block.key)).toEqual([group.key, left.key, outside.key, right.key]);
    expect(spanOf(blockAt(after, 0))).toBe(3);
  });

  it('appends a block dropped on an empty slot', () => {
    const initial = load([
      rawBlock('blockGroup', { span: 1 }, {
        'o:layout_data': {
          romeka: { block: 'blockGroup', v: 2, settings: { section: { layout: 'split-40-60' } } },
        },
      }),
      rawBlock('media', {}),
      rawBlock('html', { html: '<p>Text</p>' }),
    ]);
    const [group, media, text] = [0, 1, 2].map((index) => blockAt(initial.blocks, index));
    if (group === undefined || media === undefined || text === undefined) {
      throw new Error('three blocks');
    }
    const editor = renderEditor(initial);
    const transfer = { setData: () => undefined, effectAllowed: 'move' };
    const empty = screen.getByText('Slot 2 is empty. Drag a block here.');

    fireEvent.dragStart(card(text.key), { dataTransfer: transfer });
    fireEvent.drop(empty, { dataTransfer: transfer });

    expect(spanOf(blockAt(editor.current().blocks, 0))).toBe(2);
    expect(screen.queryByText(/is empty/)).toBeNull();
  });
});

describe('the section templates in the block picker', () => {
  it('builds Media and text: a 40/60 split holding a media block and an HTML block', () => {
    const editor = renderEditor(load([]));
    fireEvent.click(screen.getByRole('button', { name: /Insert block/ }));
    const picker = screen.getByRole('dialog');
    expect(within(picker).getByRole('heading', { name: 'Sections' })).toBeInTheDocument();
    fireEvent.click(within(picker).getByRole('button', { name: /^Media and text/ }));

    const blocks = editor.current().blocks;
    expect(blocks.map((block) => block.layout)).toEqual(['blockGroup', 'media', 'html']);
    expect(spanOf(blockAt(blocks, 0))).toBe(2);
    expect(stored(blockAt(blocks, 0), 'romeka.section.layout')).toBe('split-40-60');
  });

  it('offers the five sections the brief names', () => {
    renderEditor(load([]));
    fireEvent.click(screen.getByRole('button', { name: /Insert block/ }));
    const heading = screen.getByRole('heading', { name: 'Sections' });
    const list = heading.nextElementSibling;
    expect(list).toBeInstanceOf(HTMLElement);
    const names = within(list instanceof HTMLElement ? list : document.body)
      .getAllByRole('button')
      .map((button) => button.querySelector('.canvas-picker-name')?.textContent);
    expect(names).toEqual([
      'Media and text',
      'Text and media',
      'Three cards',
      'Quote band',
      'Full-width image band',
    ]);
  });
});

describe('the shared styling', () => {
  const openStyling = async (initial: PageDraft, index: number): Promise<HTMLElement> => {
    fireEvent.click(card(blockAt(initial.blocks, index).key));
    return within(inspector()).findByRole('group', { name: 'Block styling' });
  };

  it('turns a typed anchor into a link name and flags one the page already uses', async () => {
    const initial = load([
      rawBlock('html', { html: '<p>One</p>' }, {
        'o:layout_data': { romeka: { block: 'html', v: 2, settings: { anchor: 'our-story' } } },
      }),
      rawBlock('html', { html: '<p>Two</p>' }),
    ]);
    const editor = renderEditor(initial);
    const styling = await openStyling(initial, 1);
    fireEvent.click(within(styling).getByText('Advanced'));
    const input = within(styling).getByLabelText('Link name');

    fireEvent.change(input, { target: { value: 'Our Story!' } });

    expect(stored(blockAt(editor.current().blocks, 1), 'romeka.anchor')).toBe('our-story');
    expect(within(styling).getByRole('alert')).toHaveTextContent(
      'Another block on this page uses this name.',
    );
    fireEvent.change(input, { target: { value: 'Our Team' } });
    expect(stored(blockAt(editor.current().blocks, 1), 'romeka.anchor')).toBe('our-team');
    expect(within(styling).queryByRole('alert')).toBeNull();
    expect(within(styling).getByText('Link to it with #our-team.')).toBeInTheDocument();
  });

  it('stores a theme background as a role and a hex background upstream, never both', async () => {
    const initial = load([
      rawBlock('html', { html: '<p>One</p>' }, { 'o:layout_data': { background_color: '#336699' } }),
    ]);
    const editor = renderEditor(initial);
    const styling = await openStyling(initial, 0);
    const background = within(styling).getByRole('radiogroup', {
      name: 'Theme colors for Background color',
    });

    fireEvent.click(within(background).getByRole('radio', { name: 'Surface' }));

    const themed = layoutDataObject(blockAt(editor.current().blocks, 0));
    expect(themed.background_color).toBe('');
    expect(themed.romeka).toEqual({ block: 'html', v: 2, settings: { background_role: 'surface' } });

    fireEvent.change(within(styling).getByLabelText('Background color'), {
      target: { value: '#112233' },
    });
    const hex = layoutDataObject(blockAt(editor.current().blocks, 0));
    expect(hex.background_color).toBe('#112233');
    expect(hex.romeka).toBeUndefined();
  });

  it('writes the spacing scale and full width under romeka', async () => {
    const initial = load([rawBlock('html', { html: '<p>One</p>' })]);
    const editor = renderEditor(initial);
    const styling = await openStyling(initial, 0);
    fireEvent.change(within(styling).getByLabelText('Space above'), { target: { value: 'none' } });
    fireEvent.click(within(styling).getByLabelText('Full width'));
    expect(layoutDataObject(blockAt(editor.current().blocks, 0)).romeka).toEqual({
      block: 'html',
      v: 2,
      settings: { space_top: 'none', bleed: true },
    });
  });
});

describe('layout-data paths', () => {
  const html = { layout: 'html', version: 2 };

  it('puts styling in a carrier settings, beside a Romeka block own settings', () => {
    const hero = { romeka: { block: 'hero', v: 2, settings: { heading: 'Hi' } } };
    expect(writePaths(hero, { 'romeka.anchor': 'top' }, html)).toEqual({
      romeka: { block: 'hero', v: 2, settings: { heading: 'Hi', anchor: 'top' } },
    });
  });

  it('creates a carrier naming the block layout, and drops it when cleared', () => {
    const written = writePaths({ class: 'x' }, { 'romeka.anchor': 'top' }, html);
    expect(written).toEqual({
      class: 'x',
      romeka: { block: 'html', v: 2, settings: { anchor: 'top' } },
    });
    expect(writePaths(written, { 'romeka.anchor': '' }, html)).toEqual({ class: 'x' });
  });

  it('draws every section layout the registry offers', () => {
    const field = BLOCK_REGISTRY.sectionLayoutData.fields.find(
      (entry) => entry.key === 'romeka.section.layout',
    );
    const offered = field?.type === 'choice' ? field.options.map((option) => option.value) : [];
    expect(offered).toHaveLength(8);
    expect(offered.filter((value) => !SECTION_SHAPES.has(value))).toEqual([]);
  });
});
