/**
 * The `layout_data` form: the styling and grid fields dispatch their key and
 * value, from the same field controls the block data uses.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { vi } from 'vitest';
import type { JsonValue } from '../src/api/types';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { LayoutDataForm } from '../src/pages/sites/editor/fields/LayoutDataForm';
import { SITE_ID } from './editor-fields-helpers';

const renderLayout = (values: Readonly<Record<string, JsonValue>>) => {
  const onChange = vi.fn<(key: string, value: JsonValue) => void>();
  render(
    <LayoutDataForm
      idPrefix="block-b0-layout"
      legend="Block styling"
      schema={BLOCK_REGISTRY.blockLayoutData}
      values={values}
      onChange={onChange}
      siteId={SITE_ID}
    />,
  );
  return onChange;
};

describe('the layout data form', () => {
  it('a text field dispatches its key and value', () => {
    const onChange = renderLayout({ class: '' });

    fireEvent.change(screen.getByLabelText('Class'), { target: { value: 'hero' } });

    expect(onChange).toHaveBeenCalledWith('class', 'hero');
  });

  it('a choice field dispatches its key and value', () => {
    const onChange = renderLayout({ alignment_block: '' });

    fireEvent.change(screen.getByLabelText('Block alignment'), { target: { value: 'center' } });

    expect(onChange).toHaveBeenCalledWith('alignment_block', 'center');
  });

  it('renders the fields of whatever schema it is given', () => {
    const onChange = vi.fn<(key: string, value: JsonValue) => void>();
    render(
      <LayoutDataForm
        idPrefix="page-layout"
        legend="Page styling"
        schema={BLOCK_REGISTRY.pageLayoutData}
        values={{ grid_columns: 0 }}
        onChange={onChange}
        siteId={SITE_ID}
      />,
    );

    fireEvent.change(screen.getByLabelText('Columns'), { target: { value: '3' } });

    expect(onChange).toHaveBeenCalledWith('grid_columns', 3);
  });

  it('labels the group from the catalogue when no legend is given', () => {
    const onChange = vi.fn<(key: string, value: JsonValue) => void>();
    render(
      <LayoutDataForm
        idPrefix="block-b0-layout"
        schema={BLOCK_REGISTRY.blockLayoutData}
        values={{}}
        onChange={onChange}
        siteId={SITE_ID}
      />,
    );

    expect(screen.getByRole('group', { name: 'Block styling' })).toBeInTheDocument();
  });
});
