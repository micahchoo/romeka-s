/**
 * The Romeka carrier on the canvas: the picker inserts the upstream carrier,
 * the card shows the Romeka block's label, and a plain rendering edited outside
 * Romeka shows the badge and the restore action.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { useReducer, useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { draftReducer } from '../src/pages/sites/editor/draft';
import type { BlockKey, PageDraft } from '../src/pages/sites/editor/draft';
import { PageCanvas } from '../src/pages/sites/editor/canvas';
import { plainHash } from '../src/pages/sites/editor/romekaHash';
import { blockAt, load, rawBlock } from './editor-draft-helpers';

const Harness = ({ initial }: { readonly initial: PageDraft }): ReactElement => {
  const [draft, dispatch] = useReducer(draftReducer, initial);
  const [selected, setSelected] = useState<BlockKey | null>(null);
  return (
    <PageCanvas
      draft={draft}
      siteSettings={null}
      selectedKey={selected}
      onSelect={setSelected}
      dispatch={dispatch}
    />
  );
};

const card = (key: string): HTMLElement => {
  const element = document.querySelector(`[data-key="${key}"]`);
  if (!(element instanceof HTMLElement)) {
    throw new Error(`no card for ${key}`);
  }
  return element;
};

const editedCarrier = (): PageDraft =>
  load([
    rawBlock('html', { html: '<p>changed outside</p>' }, {
      'o:layout_data': {
        romeka: {
          block: 'hero',
          v: 1,
          settings: { heading: 'Welcome' },
          plain_hash: plainHash('<h2>Welcome</h2>'),
        },
      },
    }),
  ]);

describe('the block picker and the Romeka carrier', () => {
  it('inserts the upstream carrier carrying the Romeka block', () => {
    render(<Harness initial={load([])} />);
    fireEvent.click(screen.getByRole('button', { name: /Insert block/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Hero banner' }));
    expect(screen.getByText('Hero banner')).toBeInTheDocument();
    expect(document.querySelector('li.canvas-card')).not.toBeNull();
  });

  it('shows the Romeka label on the card of a stored carrier', () => {
    const draft = load([
      rawBlock('html', { html: '<h2>Welcome</h2>' }, {
        'o:layout_data': {
          romeka: { block: 'hero', v: 1, settings: { heading: 'Welcome' } },
        },
      }),
    ]);
    render(<Harness initial={draft} />);
    expect(card(blockAt(draft.blocks, 0).key)).toHaveTextContent('Hero banner');
  });
});

describe('a carrier edited outside Romeka', () => {
  it('shows the badge and restores the Romeka version', () => {
    const draft = editedCarrier();
    const key = blockAt(draft.blocks, 0).key;
    render(<Harness initial={draft} />);
    expect(card(key)).toHaveTextContent('Edited outside Romeka');
    fireEvent.click(screen.getByRole('button', { name: 'Use the Romeka version' }));
    expect(card(key)).not.toHaveTextContent('Edited outside Romeka');
  });

  it('leaves a matching carrier unmarked', () => {
    const plain = '<h2>Welcome</h2>';
    const draft = load([
      rawBlock('html', { html: plain }, {
        'o:layout_data': {
          romeka: { block: 'hero', v: 1, settings: { heading: 'Welcome' }, plain_hash: plainHash(plain) },
        },
      }),
    ]);
    render(<Harness initial={draft} />);
    expect(card(blockAt(draft.blocks, 0).key)).not.toHaveTextContent('Edited outside Romeka');
  });
});
