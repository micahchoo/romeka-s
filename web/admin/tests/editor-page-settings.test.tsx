/**
 * The page settings panel: the page's own members. The layout and grid controls
 * live here, and each edit leaves as a `DraftAction` the shell's reducer takes.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PageSettings } from '../src/pages/sites/editor/shell/PageSettings';
import { load, rawBlock } from './editor-draft-helpers';

describe('the page settings panel', () => {
  it('switches the page to the grid layout', () => {
    const dispatch = vi.fn();
    render(
      <PageSettings
        draft={load([rawBlock('html', { html: 'a' })])}
        dispatch={dispatch}
        issues={[]}
        pageId={null}
        onDeleted={vi.fn()}
        onOpenSaveTemplate={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByLabelText('Layout'), { target: { value: 'grid' } });
    expect(dispatch).toHaveBeenCalledWith({ type: 'setPageLayout', layout: 'grid' });
  });

  it('sets the column count and the gaps while the page is a grid', () => {
    const dispatch = vi.fn();
    render(
      <PageSettings
        draft={load([rawBlock('html', { html: 'a' })], {
          'o:layout': 'grid',
          'o:layout_data': { grid_columns: 12 },
        })}
        dispatch={dispatch}
        issues={[]}
        pageId={null}
        onDeleted={vi.fn()}
        onOpenSaveTemplate={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByLabelText('Columns'), { target: { value: '6' } });
    expect(dispatch).toHaveBeenCalledWith({
      type: 'setPageLayoutData',
      key: 'grid_columns',
      value: 6,
    });

    fireEvent.change(screen.getByLabelText('Column gap (px)'), { target: { value: '20' } });
    expect(dispatch).toHaveBeenCalledWith({
      type: 'setPageLayoutData',
      key: 'grid_column_gap',
      value: 20,
    });

    fireEvent.change(screen.getByLabelText('Row gap (px)'), { target: { value: '20' } });
    expect(dispatch).toHaveBeenCalledWith({
      type: 'setPageLayoutData',
      key: 'grid_row_gap',
      value: 20,
    });
  });

  it('hides the grid fields while the page is normal', () => {
    render(
      <PageSettings
        draft={load([rawBlock('html', { html: 'a' })])}
        dispatch={vi.fn()}
        issues={[]}
        pageId={null}
        onDeleted={vi.fn()}
        onOpenSaveTemplate={vi.fn()}
      />,
    );

    expect(screen.queryByLabelText('Columns')).toBeNull();
  });
});
