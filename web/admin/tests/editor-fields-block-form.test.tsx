/**
 * The scalar block field controls, through `BlockForm`: text, integer, boolean
 * and choice dispatch the action the draft model accepts, an unknown layout
 * keeps its data and says so, and a `validate()` issue renders on its field.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { BlockForm } from '../src/pages/sites/editor/fields/BlockForm';
import type { BlockDraft, ValidationIssue } from '../src/pages/sites/editor/draft';
import { actionsOf, dispatchSpy, draftFor, schemaForLayout, SITE_ID } from './editor-fields-helpers';
import { find, load, rawBlock } from './editor-draft-helpers';
import { installFetch, jsonResponse } from './stub';

interface Rendered {
  readonly dispatch: ReturnType<typeof dispatchSpy>;
  readonly block: BlockDraft;
}

const renderBlock = (
  block: BlockDraft,
  layout: string,
  issues: readonly ValidationIssue[] = [],
): Rendered => {
  const dispatch = dispatchSpy();
  render(
    <BlockForm
      block={block}
      schema={schemaForLayout(layout)}
      issues={issues}
      siteId={SITE_ID}
      dispatch={dispatch}
    />,
  );
  return { dispatch, block };
};

describe('the block form scalar controls', () => {
  it('text dispatches setField with the typed value', () => {
    const block = draftFor('oembed', { url: '', refresh: false });
    const dispatch = renderBlock(block, 'oembed').dispatch;

    fireEvent.change(screen.getByLabelText('Media URL'), {
      target: { value: 'https://example.org/watch' },
    });

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'url', value: 'https://example.org/watch' },
    ]);
  });

  it('boolean dispatches setField with the checkbox state', () => {
    const block = draftFor('oembed', { url: '', refresh: false });
    const dispatch = renderBlock(block, 'oembed').dispatch;

    fireEvent.click(screen.getByLabelText('Fetch the embed again'));

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'refresh', value: true },
    ]);
  });

  it('integer dispatches a number, and a cleared control dispatches null', () => {
    const block = draftFor('tableOfContents', { depth: 1 });
    const dispatch = renderBlock(block, 'tableOfContents').dispatch;

    fireEvent.change(screen.getByLabelText('Levels of sub-pages'), { target: { value: '3' } });
    fireEvent.change(screen.getByLabelText('Levels of sub-pages'), { target: { value: '' } });

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'depth', value: 3 },
      { type: 'setField', blockKey: block.key, field: 'depth', value: null },
    ]);
  });

  it('a single choice dispatches the option value', () => {
    const block = draftFor('lineBreak', { break_type: 'transparent' });
    const dispatch = renderBlock(block, 'lineBreak').dispatch;

    fireEvent.change(screen.getByLabelText('Style'), { target: { value: 'opaque' } });

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'break_type', value: 'opaque' },
    ]);
  });

  it('a multiple choice dispatches the checked list', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'unused' } }, { status: 500 }));
    const block = draftFor('browsePreview', { resource_type: 'items', query: '', components: [] });
    const dispatch = renderBlock(block, 'browsePreview').dispatch;

    fireEvent.click(screen.getByLabelText('Thumbnail'));

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'components', value: ['thumbnail'] },
    ]);
    await waitFor(() => {
      expect(screen.getByText('The match count could not be read.')).toBeInTheDocument();
    });
  });

  it('an unknown layout keeps its data and says it has no form', () => {
    const block = draftFor('myModuleLayout', { anything: 1 });
    const dispatch = dispatchSpy();
    render(
      <BlockForm block={block} schema={undefined} issues={[]} siteId={SITE_ID} dispatch={dispatch} />,
    );

    expect(
      screen.getByText(
        'This layout has no editable fields in this version, so its stored data is kept as it is.',
      ),
    ).toBeInTheDocument();
    expect(dispatch).not.toHaveBeenCalled();
  });

  it('a required issue renders on its own field', () => {
    const block = draftFor('oembed', { url: '' });
    renderBlock(block, 'oembed', [{ kind: 'required', blockKey: block.key, field: 'url' }]);

    expect(screen.getByText('This field is required.')).toBeInTheDocument();
    expect(screen.getByLabelText('Media URL')).toHaveAttribute('aria-invalid', 'true');
  });

  it('an issue for another block does not render', () => {
    const state = load([rawBlock('oembed', { url: '' }), rawBlock('html', {})]);
    const block = find(state, 'oembed');
    const other = find(state, 'html');
    renderBlock(block, 'oembed', [{ kind: 'required', blockKey: other.key, field: 'url' }]);

    expect(screen.queryByText('This field is required.')).not.toBeInTheDocument();
  });

  it('an integer issue renders its bound', () => {
    const block = draftFor('tableOfContents', { depth: 0 });
    renderBlock(block, 'tableOfContents', [
      { kind: 'min', blockKey: block.key, field: 'depth', min: 1 },
    ]);

    expect(screen.getByText('Enter 1 or more.')).toBeInTheDocument();
  });
});
