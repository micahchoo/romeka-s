/**
 * The resource page block editor: the ordered list per resource and the
 * show, hide and reorder gestures, each leaving the whole draft through
 * `onChange`.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ResourcePageBlocksField } from '../src/pages/sites/ResourcePageBlocksField';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from '../src/pages/sites/resourcePageBlocksRegistry';
import type { ResourcePageBlocksDraft } from '../src/pages/sites/resourcePageBlocksSchema';

const renderField = (
  drafts: ResourcePageBlocksDraft,
): ReturnType<typeof vi.fn> => {
  const onChange = vi.fn();
  render(
    <ResourcePageBlocksField
      registry={RESOURCE_PAGE_BLOCKS_REGISTRY}
      drafts={drafts}
      onChange={onChange}
    />,
  );
  return onChange;
};

const items = (): HTMLElement => screen.getByRole('group', { name: 'Items' });

describe('the resource page block editor', () => {
  it('lists each resource with its blocks in order', () => {
    renderField({
      items: ['values', 'itemSets'],
      item_sets: [],
      media: ['mediaRender'],
    });

    expect(within(items()).getAllByText(/^(Values|Item sets)$/)).toHaveLength(2);
    // An empty resource says so rather than showing an empty list.
    expect(within(screen.getByRole('group', { name: 'Item sets' })).getByText(/No sections shown/)).toBeInTheDocument();
  });

  it('says under each section what a reader sees', () => {
    renderField({ items: ['values'], item_sets: [], media: [] });

    const shown = within(items()).getByRole('list', { name: 'Sections shown on Items pages' });
    expect(within(shown).getByText('The metadata, field by field.')).toBeInTheDocument();
  });

  it('moves a section down with its arrow button', () => {
    const onChange = renderField({ items: ['values', 'itemSets'], item_sets: [], media: [] });

    fireEvent.click(within(items()).getByRole('button', { name: 'Move Values down' }));

    expect(onChange).toHaveBeenCalledWith({
      items: ['itemSets', 'values'],
      item_sets: [],
      media: [],
    });
  });

  it('moves a section up by its handle and the arrow keys', () => {
    const onChange = renderField({ items: ['values', 'itemSets'], item_sets: [], media: [] });

    const handle = within(items()).getByRole('button', { name: 'Move Item sets, 2 of 2' });
    fireEvent.click(handle);
    fireEvent.keyDown(handle, { key: 'ArrowUp' });

    expect(onChange).toHaveBeenCalledWith({
      items: ['itemSets', 'values'],
      item_sets: [],
      media: [],
    });
  });

  it('hides a section, which then sits in the hidden list', () => {
    const onChange = renderField({ items: ['values', 'itemSets'], item_sets: [], media: [] });

    fireEvent.click(within(items()).getByRole('button', { name: 'Hide Values' }));

    expect(onChange).toHaveBeenCalledWith({ items: ['itemSets'], item_sets: [], media: [] });
  });

  it('lists every compatible section that is not shown, and Show puts it at the end', () => {
    const onChange = renderField({ items: ['values'], item_sets: [], media: [] });

    const hidden = within(items()).getByRole('list', { name: 'Hidden sections of Items pages' });
    expect(within(hidden).queryByText('Values')).toBeNull();
    expect(within(hidden).getByText('Media list')).toBeInTheDocument();
    expect(within(items()).queryByRole('combobox')).toBeNull();
    fireEvent.click(within(hidden).getByRole('button', { name: 'Show Media list' }));

    expect(onChange).toHaveBeenCalledWith({
      items: ['values', 'mediaList'],
      item_sets: [],
      media: [],
    });
  });

  it('labels a stored layout the registry does not know', () => {
    renderField({ items: ['someRemovedLayout'], item_sets: [], media: [] });

    expect(within(items()).getByText('Unknown section [someRemovedLayout]')).toBeInTheDocument();
  });
});
