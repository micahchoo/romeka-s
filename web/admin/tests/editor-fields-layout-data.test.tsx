/**
 * The `layout_data` form: the styling fields sit in named sections, the grid
 * section applies only on a grid page, the template and class wait under
 * "Advanced", and each control writes its keys as one patch. Padding is one
 * control over four stored keys; the background image's fit and anchor show
 * once an image is set.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { vi } from 'vitest';
import type { JsonValue } from '../src/api/types';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { LayoutDataForm } from '../src/pages/sites/editor/fields/LayoutDataForm';
import { SITE_ID } from './editor-fields-helpers';

type Patch = Readonly<Record<string, JsonValue>>;

const renderLayout = (values: Patch, grid = false) => {
  const onChange = vi.fn<(values: Patch) => void>();
  render(
    <LayoutDataForm
      idPrefix="block-b0-layout"
      owner="block"
      schema={BLOCK_REGISTRY.blockLayoutData}
      values={values}
      onChange={onChange}
      grid={grid}
      siteId={SITE_ID}
    />,
  );
  return onChange;
};

describe('the layout data form', () => {
  it('labels the form from the catalogue and shows the named sections', () => {
    renderLayout({});

    const form = screen.getByRole('group', { name: 'Block styling' });
    for (const name of ['Spacing', 'Size', 'Background', 'Alignment', 'Grid position']) {
      expect(within(form).getByRole('group', { name })).toBeInTheDocument();
    }
  });

  it('a choice field writes its key and value', () => {
    const onChange = renderLayout({ alignment_block: '' });

    fireEvent.change(screen.getByLabelText('Block position'), { target: { value: 'center' } });

    expect(onChange).toHaveBeenCalledWith({ alignment_block: 'center' });
  });

  it('a choice with no default can go back to not set', () => {
    const onChange = renderLayout({ alignment_text: 'center' });

    const select = screen.getByLabelText('Text alignment');
    expect(select).toHaveValue('center');
    fireEvent.change(select, { target: { value: '' } });

    expect(within(select).getByRole('option', { name: 'Not set' })).toBeInTheDocument();
    expect(onChange).toHaveBeenCalledWith({ alignment_text: '' });
  });

  it('every control carries its help line', () => {
    renderLayout({});

    expect(screen.getByLabelText('Maximum width')).toHaveAccessibleDescription(
      'A number alone is pixels; % and rem also work.',
    );
  });

  it('keeps the template and class under a closed Advanced disclosure', () => {
    const onChange = renderLayout({ class: '' });

    const advanced = screen.getByText('Advanced').closest('details');
    expect(advanced).not.toBeNull();
    expect(advanced).not.toHaveAttribute('open');
    fireEvent.change(within(advanced ?? document.body).getByLabelText('CSS class'), {
      target: { value: 'hero' },
    });

    expect(onChange).toHaveBeenCalledWith({ class: 'hero' });
  });

  it('disables the grid section with a reason on a page that is not a grid', () => {
    renderLayout({ grid_column_span: '2' });

    const section = screen.getByRole('group', { name: 'Grid position' });
    expect(section).toBeDisabled();
    expect(within(section).getByText('Only on grid pages.')).toBeInTheDocument();
    expect(within(section).getByLabelText('Columns wide')).toHaveValue('2');
  });

  it('enables the grid section on a grid page', () => {
    renderLayout({}, true);

    expect(screen.getByRole('group', { name: 'Grid position' })).toBeEnabled();
    expect(screen.queryByText('Only on grid pages.')).toBeNull();
  });

  it('a padding preset writes all four sides in one patch', () => {
    const onChange = renderLayout({});

    fireEvent.click(screen.getByRole('button', { name: 'Medium' }));

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith({
      padding_top: '1rem',
      padding_right: '1rem',
      padding_bottom: '1rem',
      padding_left: '1rem',
    });
  });

  it('linked padding writes all four sides, unlinked padding writes one', () => {
    const onChange = renderLayout({});

    fireEvent.change(screen.getByLabelText('All sides'), { target: { value: '12px' } });
    expect(onChange).toHaveBeenLastCalledWith({
      padding_top: '12px',
      padding_right: '12px',
      padding_bottom: '12px',
      padding_left: '12px',
    });

    fireEvent.click(screen.getByLabelText('Same on all sides'));
    fireEvent.change(screen.getByLabelText('Left'), { target: { value: '3rem' } });
    expect(onChange).toHaveBeenLastCalledWith({ padding_left: '3rem' });
  });

  it('opens unlinked on the four stored sides when they differ, and marks the matching preset', () => {
    renderLayout({ padding_top: '1rem', padding_right: '0', padding_bottom: '1rem', padding_left: '0' });

    expect(screen.getByLabelText('Top')).toHaveValue('1rem');
    expect(screen.getByLabelText('Right')).toHaveValue('0');
    expect(screen.getByRole('button', { name: 'Medium' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('the colour control writes the hex code and flags one the page would drop', () => {
    const onChange = renderLayout({ background_color: '#zz0000' });

    expect(screen.getByRole('alert')).toHaveTextContent('Use # and 3 or 6 hex digits.');
    fireEvent.change(screen.getByLabelText('Background color'), { target: { value: '#1a2b3c' } });
    // A hex colour replaces a theme colour, so the write clears the role.
    expect(onChange).toHaveBeenCalledWith({ background_color: '#1a2b3c', 'romeka.background_role': '' });
    const field = screen.getByLabelText('Background color').closest('.block-field');
    if (!(field instanceof HTMLElement)) {
      throw new Error('the background colour field');
    }
    fireEvent.click(within(field).getByRole('button', { name: 'No color' }));
    expect(onChange).toHaveBeenLastCalledWith({ background_color: '', 'romeka.background_role': '' });
  });

  it('hides the image fit and anchors while no background image is set', () => {
    renderLayout({});
    expect(screen.queryByLabelText('Image fit')).toBeNull();
  });

  it('shows the image fit and anchors for a stored background image', () => {
    renderLayout({ background_image_asset: '4' });
    expect(screen.getByLabelText('Image fit')).toBeInTheDocument();
    expect(screen.getByLabelText('Image anchor (vertical)')).toBeInTheDocument();
  });

  it('renders the fields of whatever schema it is given', () => {
    const onChange = vi.fn<(values: Patch) => void>();
    render(
      <LayoutDataForm
        idPrefix="page-layout"
        owner="page"
        schema={BLOCK_REGISTRY.pageLayoutData}
        values={{ grid_columns: 0 }}
        onChange={onChange}
        grid
        siteId={SITE_ID}
      />,
    );

    fireEvent.change(screen.getByLabelText('Columns'), { target: { value: '3' } });

    expect(onChange).toHaveBeenCalledWith({ grid_columns: 3 });
  });
});
