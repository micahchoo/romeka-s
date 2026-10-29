/**
 * Every count the page canvas shows reads "1 block", "1 issue", never
 * "1 blocks": the editor picks the singular message for a count of one.
 */
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { PageCanvas } from '../src/pages/sites/editor/canvas';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { blockAt, load, rawBlock } from './editor-draft-helpers';
import { SITE_ID } from './editor-fields-helpers';

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

const ATTACHMENT = { 'o:item': null, 'o:media': null, 'o:caption': '' };

describe('the canvas counts', () => {
  it('counts one block in the singular', () => {
    renderCanvas(load([rawBlock('html', { html: 'a' })]));
    expect(screen.getByText('1 block')).toBeInTheDocument();
  });

  it('names one issue on a card in the singular, in its badge and its label', () => {
    // An oEmbed block without a URL has exactly one issue.
    const draft = load([rawBlock('oembed', {})]);
    renderCanvas(draft);
    const oembed = card(blockAt(draft.blocks, 0).key);
    expect(oembed).toHaveTextContent('1 issue');
    expect(oembed).not.toHaveTextContent('1 issues');
    expect(oembed).toHaveAccessibleName('Block 1: oEmbed, 1 issue');
  });

  it('counts one attachment in the singular', () => {
    const draft = load([rawBlock('media', {}, { 'o:attachment': [ATTACHMENT] })]);
    renderCanvas(draft);
    expect(card(blockAt(draft.blocks, 0).key)).toHaveTextContent('1 attachment');
    expect(card(blockAt(draft.blocks, 0).key)).not.toHaveTextContent('1 attachments');
  });

  it('labels a group of one block in the singular', () => {
    renderCanvas(
      load([rawBlock('blockGroup', { span: '1' }), rawBlock('html', { html: 'member' })]),
    );
    expect(screen.getByText('Group, 1 block')).toBeInTheDocument();
  });

  it('keeps the plural for every other count', () => {
    const draft = load([
      rawBlock('media', {}, { 'o:attachment': [ATTACHMENT, ATTACHMENT] }),
      rawBlock('html', { html: 'b' }),
    ]);
    renderCanvas(draft);
    expect(screen.getByText('2 blocks')).toBeInTheDocument();
    expect(card(blockAt(draft.blocks, 0).key)).toHaveTextContent('2 attachments');
  });
});
