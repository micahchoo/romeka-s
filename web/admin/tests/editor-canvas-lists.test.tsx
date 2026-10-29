/**
 * A list block's Outline row says "Lists nothing yet" when its query matches
 * nothing on the site, from the same count the inspector reads.
 */
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { PageCanvas } from '../src/pages/sites/editor/canvas';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { blockAt, load, rawBlock } from './editor-draft-helpers';
import { SITE_ID } from './editor-fields-helpers';
import { installFetch, listResponse, requestsFor } from './stub';

const renderCanvas = (draft: PageDraft): void => {
  render(
    <PageCanvas
      draft={draft}
      siteSettings={null}
      siteId={SITE_ID}
      selection={null}
      onSelect={() => {}}
      dispatch={() => {}}
    />,
  );
};

const card = (key: string): HTMLElement => {
  const element = document.querySelector(`[data-key="${key}"]`);
  if (!(element instanceof HTMLElement)) {
    throw new Error(`no card for ${key}`);
  }
  return element;
};

describe('a list block row', () => {
  it('says it lists nothing yet when no item set of the site matches', async () => {
    const fetchMock = installFetch(() => listResponse([], 0));
    const draft = load([
      rawBlock('browsePreview', { resource_type: 'item_sets', query: '', components: [] }),
    ]);
    renderCanvas(draft);

    expect(await screen.findByText('Lists nothing yet')).toBeInTheDocument();
    expect(card(blockAt(draft.blocks, 0).key)).toHaveTextContent('Lists nothing yet');
    const counted = requestsFor(fetchMock, '/api/item_sets');
    expect(counted[0]?.url.searchParams.get('site_id')).toBe(String(SITE_ID));
  });

  it('says nothing when the query matches', async () => {
    const fetchMock = installFetch(() => listResponse([], 4));
    const draft = load([
      rawBlock('browsePreview', { resource_type: 'items', query: '', components: [] }),
    ]);
    renderCanvas(draft);

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/items').length).toBeGreaterThan(0);
    });
    // The answer is read after the request goes out: let it land.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByText('Lists nothing yet')).toBeNull();
  });
});

