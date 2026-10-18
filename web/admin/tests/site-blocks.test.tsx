/**
 * The block editor reads its layout list from the generated registry
 * (`src/generated/blocks.json`), so a layout the Rust registry declares and the
 * admin stops offering fails here.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ParseError } from '../src/api/parse';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { parseBlockRegistry } from '../src/pages/sites/blockSchema';
import { BlockEditor } from '../src/pages/sites/blocks';
import type { BlockDraft } from '../src/pages/sites/blocks';

/**
 * The fifteen registered layouts, in upstream's order
 * (`application/config/module.config.php:805-825`).
 */
const REGISTERED = [
  'pageTitle',
  'media',
  'browsePreview',
  'listOfSites',
  'tableOfContents',
  'lineBreak',
  'itemWithMetadata',
  'pageDateTime',
  'blockGroup',
  'iiifPresentation',
  'iiifImage',
  'asset',
  'html',
  'listOfPages',
  'oembed',
] as const;

const draft = (layout: string): BlockDraft => ({
  key: 0,
  layout,
  data: '{}',
  layoutData: '',
});

const optionValues = (): readonly (string | null)[] =>
  within(screen.getByLabelText('Block 1 layout'))
    .getAllByRole('option')
    .map((option) => option.getAttribute('value'));

describe('the block layout select', () => {
  it('lists every generated layout, in the registry order', () => {
    render(<BlockEditor blocks={[draft('html')]} onChange={() => {}} />);
    expect(BLOCK_REGISTRY.layouts.map((layout) => layout.name)).toEqual([...REGISTERED]);
    expect(optionValues()).toEqual([...REGISTERED]);
  });

  it('shows a layout under its catalogue label', () => {
    render(<BlockEditor blocks={[draft('html')]} onChange={() => {}} />);
    expect(screen.getByRole('option', { name: 'HTML' }).getAttribute('value')).toBe('html');
    expect(
      screen.getByRole('option', { name: 'Browse preview' }).getAttribute('value'),
    ).toBe('browsePreview');
  });

  it('keeps a stored layout the registry does not register', () => {
    render(<BlockEditor blocks={[draft('myModuleLayout')]} onChange={() => {}} />);
    expect(
      screen.getByRole('option', { name: 'Unknown [myModuleLayout]' }).getAttribute('value'),
    ).toBe('myModuleLayout');
    expect(optionValues()).toEqual([...REGISTERED, 'myModuleLayout']);
  });
});

describe('the registry boundary', () => {
  it('rejects a field label the catalogue does not carry', () => {
    expect(() =>
      parseBlockRegistry({
        layouts: [
          {
            name: 'html',
            label: 'layout.html',
            fields: [{ key: 'html', label: 'blockField.html.missing', type: 'html' }],
          },
        ],
        blockLayoutData: { fields: [] },
        pageLayoutData: { fields: [] },
      }),
    ).toThrow(ParseError);
  });
});
