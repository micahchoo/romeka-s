/**
 * The shared reorder model: the pure move, and the hook driven the way a
 * curator drives a list, by the handle's keys, by the step buttons and by a
 * drag, with each step read back from the list and its live region.
 */
import { createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { moveItem } from '../src/app/reorder/move';
import {
  ReorderButtons,
  ReorderHandle,
  ReorderStatus,
  useReorderable,
} from '../src/app/reorder/useReorderable';

describe('moveItem', () => {
  it('moves an entry to a later place', () => {
    expect(moveItem(['a', 'b', 'c', 'd'], 0, 2)).toEqual(['b', 'c', 'a', 'd']);
  });

  it('moves an entry to an earlier place', () => {
    expect(moveItem(['a', 'b', 'c', 'd'], 3, 1)).toEqual(['a', 'd', 'b', 'c']);
  });

  it('clamps a target past either end', () => {
    expect(moveItem(['a', 'b', 'c'], 1, 9)).toEqual(['a', 'c', 'b']);
    expect(moveItem(['a', 'b', 'c'], 1, -4)).toEqual(['b', 'a', 'c']);
  });

  it('returns the same list when nothing moves', () => {
    const list = ['a', 'b'];
    expect(moveItem(list, 0, 0)).toBe(list);
    expect(moveItem(list, 0, -1)).toBe(list);
    expect(moveItem(list, 5, 0)).toBe(list);
  });
});

const Fruit = ({ initial }: { readonly initial: readonly string[] }): ReactElement => {
  const [items, setItems] = useState(initial);
  const reorder = useReorderable({ items, onReorder: setItems, labelOf: (item) => item });
  return (
    <>
      <ol aria-label="Fruit">
        {items.map((item, index) => (
          <li key={item} {...reorder.row(index)}>
            <ReorderHandle reorder={reorder} index={index} />
            <span>{item}</span>
            <ReorderButtons reorder={reorder} index={index} />
          </li>
        ))}
      </ol>
      <ReorderStatus reorder={reorder} />
    </>
  );
};

const order = (): readonly string[] =>
  within(screen.getByRole('list', { name: 'Fruit' }))
    .getAllByRole('listitem')
    .map((row) => row.querySelector('span:not([aria-hidden])')?.textContent ?? '');

const handle = (name: string): HTMLElement =>
  screen.getByRole('button', { name: new RegExp(`^Move ${name}, `) });

describe('useReorderable', () => {
  it('picks a row up with its handle, moves it with the arrows and drops it', () => {
    render(<Fruit initial={['apple', 'pear', 'plum']} />);

    fireEvent.click(handle('apple'));
    expect(screen.getByRole('status')).toHaveTextContent('Picked up apple.');
    fireEvent.keyDown(handle('apple'), { key: 'ArrowDown' });
    fireEvent.keyDown(handle('apple'), { key: 'ArrowDown' });

    expect(order()).toEqual(['pear', 'plum', 'apple']);
    expect(screen.getByRole('status')).toHaveTextContent('apple: position 3 of 3.');
    expect(handle('apple')).toHaveFocus();

    fireEvent.click(handle('apple'));
    expect(screen.getByRole('status')).toHaveTextContent('Dropped apple at position 3 of 3.');
    // Once dropped, the arrows no longer move it.
    fireEvent.keyDown(handle('apple'), { key: 'ArrowUp' });
    expect(order()).toEqual(['pear', 'plum', 'apple']);
  });

  it('puts the list back on Escape', () => {
    render(<Fruit initial={['apple', 'pear', 'plum']} />);

    fireEvent.click(handle('pear'));
    fireEvent.keyDown(handle('pear'), { key: 'ArrowUp' });
    expect(order()).toEqual(['pear', 'apple', 'plum']);
    fireEvent.keyDown(handle('pear'), { key: 'Escape' });

    expect(order()).toEqual(['apple', 'pear', 'plum']);
    expect(screen.getByRole('status')).toHaveTextContent('Move of pear cancelled.');
    expect(handle('pear')).toHaveAttribute('aria-pressed', 'false');
  });

  it('ignores the arrows on a row that is not picked up', () => {
    render(<Fruit initial={['apple', 'pear']} />);

    fireEvent.keyDown(handle('apple'), { key: 'ArrowDown' });

    expect(order()).toEqual(['apple', 'pear']);
  });

  it('moves one place with the step buttons, which stop at the ends', () => {
    render(<Fruit initial={['apple', 'pear', 'plum']} />);

    expect(screen.getByRole('button', { name: 'Move apple up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move plum down' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Move plum up' }));

    expect(order()).toEqual(['apple', 'plum', 'pear']);
    expect(screen.getByRole('status')).toHaveTextContent('plum: position 2 of 3.');
    expect(screen.getByRole('button', { name: 'Move plum up' })).toHaveFocus();
  });

  it('moves a dragged row to the row it is dropped on', () => {
    render(<Fruit initial={['apple', 'pear', 'plum']} />);
    const store = new Map<string, string>();
    const dataTransfer = {
      get types() {
        return [...store.keys()];
      },
      setData: (type: string, value: string) => store.set(type, value),
      getData: (type: string) => store.get(type) ?? '',
      effectAllowed: 'none',
      dropEffect: 'none',
    };
    const rows = within(screen.getByRole('list', { name: 'Fruit' })).getAllByRole('listitem');
    const target = rows[2];
    if (target === undefined) {
      throw new Error('expected three rows');
    }

    fireEvent.dragStart(handle('apple'), { dataTransfer });
    const over = createEvent.dragOver(target, { dataTransfer });
    fireEvent(target, over);
    expect(over.defaultPrevented).toBe(true);
    fireEvent.drop(target, { dataTransfer });

    expect(order()).toEqual(['pear', 'plum', 'apple']);
    expect(screen.getByRole('status')).toHaveTextContent('Dropped apple at position 3 of 3.');
  });
});
