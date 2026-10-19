/**
 * The item picker: it searches as the user types, and a keyboard alone can move
 * between results and pick several of them.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactElement } from 'react';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';
import type { ItemId } from '../src/api/types';
import { ItemPicker } from '../src/pages/sites/editor/fields/ItemPicker';
import { loadFixture } from './loadFixture';
import { installFetch, listResponse, requestsFor } from './stub';

const Harness = ({ onPicked }: { readonly onPicked: (ids: readonly ItemId[]) => void }): ReactElement => {
  const [picked, setPicked] = useState<readonly ItemId[]>([]);
  return (
    <ItemPicker
      id="picker"
      label="Find items"
      picked={picked}
      onChange={(next) => {
        setPicked(next);
        onPicked(next);
      }}
    />
  );
};

describe('ItemPicker', () => {
  it('searches the typed title and picks results from the keyboard', async () => {
    const fetchMock = installFetch(() => listResponse(loadFixture('items-page.json'), 2));
    let picked: readonly ItemId[] = [];
    render(
      <Harness
        onPicked={(ids) => {
          picked = ids;
        }}
      />,
    );

    fireEvent.change(screen.getByLabelText('Find items'), { target: { value: 'fixture' } });
    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items').some(
          (request) => request.url.searchParams.get('search') === 'fixture',
        ),
      ).toBe(true);
    });
    const first = await screen.findByRole('button', { name: 'Fixture Item One' });

    first.focus();
    fireEvent.keyDown(first, { key: 'ArrowRight' });
    const second = screen.getByRole('button', { name: 'Fixture Item Two' });
    expect(second).toHaveFocus();
    fireEvent.click(second);
    fireEvent.keyDown(second, { key: 'Home' });
    expect(first).toHaveFocus();
    fireEvent.click(first);

    expect(picked.map(Number)).toEqual([9, 12]);
    expect(first).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText('Picked: 2')).toBeInTheDocument();
  });
});
