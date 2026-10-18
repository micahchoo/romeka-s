/**
 * The resource page block editor: the ordered list per resource and the
 * add, remove and reorder gestures, each leaving the whole draft through
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
    expect(within(screen.getByRole('group', { name: 'Item sets' })).getByText(/No blocks/)).toBeInTheDocument();
  });

  it('moves a block down within its resource', () => {
    const onChange = renderField({ items: ['values', 'itemSets'], item_sets: [], media: [] });

    fireEvent.click(within(items()).getByRole('button', { name: 'Move Values down' }));

    expect(onChange).toHaveBeenCalledWith({
      items: ['itemSets', 'values'],
      item_sets: [],
      media: [],
    });
  });

  it('moves a block up within its resource', () => {
    const onChange = renderField({ items: ['values', 'itemSets'], item_sets: [], media: [] });

    fireEvent.click(within(items()).getByRole('button', { name: 'Move Item sets up' }));

    expect(onChange).toHaveBeenCalledWith({
      items: ['itemSets', 'values'],
      item_sets: [],
      media: [],
    });
  });

  it('removes a block from its resource', () => {
    const onChange = renderField({ items: ['values', 'itemSets'], item_sets: [], media: [] });

    fireEvent.click(within(items()).getByRole('button', { name: 'Remove Values' }));

    expect(onChange).toHaveBeenCalledWith({ items: ['itemSets'], item_sets: [], media: [] });
  });

  it('adds a compatible layout that is not already in the list', () => {
    const onChange = renderField({ items: ['values'], item_sets: [], media: [] });

    const add = within(items()).getByRole('combobox', { name: 'Add a block to Items' });
    expect(within(add).queryByRole('option', { name: 'Values' })).toBeNull();
    fireEvent.change(add, { target: { value: 'mediaList' } });
    fireEvent.click(within(items()).getByRole('button', { name: 'Add' }));

    expect(onChange).toHaveBeenCalledWith({
      items: ['values', 'mediaList'],
      item_sets: [],
      media: [],
    });
  });

  it('labels a stored layout the registry does not know', () => {
    renderField({ items: ['someRemovedLayout'], item_sets: [], media: [] });

    expect(within(items()).getByText('Unknown block layout [someRemovedLayout]')).toBeInTheDocument();
  });
});
