/**
 * The page settings panel: the page's own members. The layout control lives
 * here, the page's layout data is the schema's `pageLayoutData` rendered by
 * the styling form, and each edit leaves as a `DraftAction` the shell's
 * reducer takes.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { en } from '../src/pages/sites/editor/messages';
import { PageSettings } from '../src/pages/sites/editor/shell/PageSettings';
import { load, rawBlock } from './editor-draft-helpers';
import { SITE_ID } from './editor-fields-helpers';

const renderSettings = (page: Readonly<Record<string, unknown>> = {}, dispatch = vi.fn()) => {
  render(
    <PageSettings
      draft={load([rawBlock('html', { html: 'a' })], page)}
      dispatch={dispatch}
      issues={[]}
      pageId={null}
      siteId={SITE_ID}
      options={{ pages: [], itemSets: [], classes: [] }}
      onDeleted={vi.fn()}
      onOpenSaveTemplate={vi.fn()}
      onSelectRegion={vi.fn()}
    />,
  );
  return dispatch;
};

/** The text of the help line a control names with `aria-describedby`. */
const helpOf = (control: HTMLElement): string => {
  const id = control.getAttribute('aria-describedby') ?? '';
  return document.getElementById(id)?.textContent ?? '';
};

describe('the page settings panel', () => {
  it('switches the page to the grid layout', async () => {
    const dispatch = renderSettings();

    fireEvent.change(await screen.findByLabelText('Layout'), { target: { value: 'grid' } });
    expect(dispatch).toHaveBeenCalledWith({ type: 'setPageLayout', layout: 'grid' });
  });

  it('renders every page layout data field of the schema, with its help, on a grid page', async () => {
    renderSettings({ 'o:layout': 'grid' });
    await screen.findByLabelText('Layout');

    const fields = BLOCK_REGISTRY.pageLayoutData.fields;
    expect(fields.length).toBeGreaterThan(0);
    for (const field of fields) {
      const control = screen.getByLabelText(en[field.label]);
      expect(helpOf(control)).toBe(en[field.help]);
    }
    const grid = screen.getByRole('group', { name: 'Grid' });
    expect(within(grid).getByLabelText('Columns')).toBeInTheDocument();
    expect(within(grid).getByLabelText('Column gap (px)')).toBeInTheDocument();
    expect(within(grid).getByLabelText('Row gap (px)')).toBeInTheDocument();
  });

  it('hides the grid fields on a normal page and keeps the advanced ones', async () => {
    renderSettings();
    await screen.findByLabelText('Layout');

    expect(screen.queryByRole('group', { name: 'Grid' })).toBeNull();
    expect(screen.queryByLabelText('Columns')).toBeNull();
    expect(screen.queryByLabelText('Column gap (px)')).toBeNull();
    expect(screen.queryByLabelText('Row gap (px)')).toBeNull();
    expect(screen.getByLabelText('Template')).toBeInTheDocument();
    expect(screen.getByLabelText('Class')).toBeInTheDocument();
  });

  it('writes the column count, the gaps and the class to their stored keys', async () => {
    const dispatch = renderSettings({ 'o:layout': 'grid', 'o:layout_data': { grid_columns: 12 } });

    const columns = await screen.findByLabelText('Columns');
    expect(columns).toHaveValue(12);
    fireEvent.change(columns, { target: { value: '6' } });
    expect(dispatch).toHaveBeenCalledWith({ type: 'setPageLayoutData', key: 'grid_columns', value: 6 });

    // An unset gap shows the renderer's 10 pixels without storing it.
    const columnGap = screen.getByLabelText('Column gap (px)');
    expect(columnGap).toHaveValue(null);
    expect(columnGap).toHaveAttribute('placeholder', '10');
    fireEvent.change(columnGap, { target: { value: '20' } });
    expect(dispatch).toHaveBeenCalledWith({
      type: 'setPageLayoutData',
      key: 'grid_column_gap',
      value: 20,
    });

    fireEvent.change(screen.getByLabelText('Row gap (px)'), { target: { value: '20' } });
    expect(dispatch).toHaveBeenCalledWith({ type: 'setPageLayoutData', key: 'grid_row_gap', value: 20 });

    fireEvent.change(screen.getByLabelText('Class'), { target: { value: 'wide' } });
    expect(dispatch).toHaveBeenCalledWith({ type: 'setPageLayoutData', key: 'class', value: 'wide' });
  });
});
