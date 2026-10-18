/**
 * The site Theme page and its pure pieces: the token schema, the WCAG contrast
 * functions, and the page that saves and previews a palette.
 *
 * The page runs through the real typed client against a stubbed fetch, so the
 * body it PATCHes and the body it POSTs to the preview are the bytes the server
 * would get.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { parseThemeSettingsRegistry } from '../src/pages/sites/themeSettingsSchema';
import {
  themeSettingDrafts,
  themeSettingsWire,
} from '../src/pages/sites/themeSettingsSchema';
import { THEME_SETTINGS_REGISTRY } from '../src/pages/sites/themeSettingsRegistry';
import {
  parseResourcePageBlocksRegistry,
  resourcePageBlockDrafts,
  resourcePageBlocksWire,
} from '../src/pages/sites/resourcePageBlocksSchema';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from '../src/pages/sites/resourcePageBlocksRegistry';
import { contrastChecks, contrastRatio, parseHex } from '../src/pages/sites/contrast';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';

describe('the theme token schema', () => {
  it('the generated registry is the Rust schema', () => {
    const keys = THEME_SETTINGS_REGISTRY.settings.map((spec) => spec.key);
    expect(keys).toContain('brand_color');
    expect(keys).toContain('footer_text');
    expect(THEME_SETTINGS_REGISTRY.settings).toHaveLength(19);
    const brand = THEME_SETTINGS_REGISTRY.settings.find((spec) => spec.key === 'brand_color');
    expect(brand?.kind).toBe('colour');
    if (brand?.kind === 'colour') {
      expect(brand.dark).toMatch(/^#/);
      expect(brand.default).toMatch(/^#/);
    }
  });

  it('a registry with an unknown label key fails to parse', () => {
    expect(() =>
      parseThemeSettingsRegistry({
        settings: [{ key: 'brand_color', labelKey: 'themeSetting.nope', kind: 'text', default: null }],
      }),
    ).toThrow();
  });

  it('drafts read stored values and take the default otherwise', () => {
    const drafts = themeSettingDrafts(THEME_SETTINGS_REGISTRY, { brand_color: '#123456', logo: 7 });
    expect(drafts['brand_color']).toBe('#123456');
    expect(drafts['logo']).toBe('7');
    expect(drafts['footer_text']).toBe('');
    // The dark scheme supplies the neutral defaults.
    const dark = themeSettingDrafts(THEME_SETTINGS_REGISTRY, { color_scheme: 'dark' });
    expect(dark['background_color']).toBe('#12161a');
    expect(dark['text_color']).toBe('#e8ecef');
  });

  it('the wire map carries each token in its storage shape', () => {
    const wire = themeSettingsWire(THEME_SETTINGS_REGISTRY, {
      ...themeSettingDrafts(THEME_SETTINGS_REGISTRY, {}),
      brand_color: '#123456',
      logo: '9',
      banner: '',
      footer_text: '<p>Hi</p>',
      base_font_size: 18,
    });
    expect(wire['brand_color']).toBe('#123456');
    expect(wire['logo']).toBe(9);
    expect(wire['banner']).toBeNull();
    expect(wire['footer_text']).toBe('<p>Hi</p>');
    expect(wire['base_font_size']).toBe(18);
    // An out-of-range number is clamped to the schema's bounds.
    const clamped = themeSettingsWire(THEME_SETTINGS_REGISTRY, {
      ...themeSettingDrafts(THEME_SETTINGS_REGISTRY, {}),
      corner_radius: 900,
    });
    expect(clamped['corner_radius']).toBe(24);
  });
});

describe('the resource page block schema', () => {
  it('the generated registry is the Rust schema', () => {
    const resources = RESOURCE_PAGE_BLOCKS_REGISTRY.resources;
    expect(resources.map((resource) => resource.key)).toEqual(['items', 'item_sets', 'media']);
    const items = resources[0];
    expect(items?.default).toEqual([
      'mediaEmbeds',
      'values',
      'itemSets',
      'sitePages',
      'mediaList',
      'linkedResources',
    ]);
    expect(items?.blocks.map((block) => block.name)).toContain('mediaList');
    // An item set page accepts no media list.
    expect(resources[1]?.blocks.map((block) => block.name)).not.toContain('mediaList');
    expect(resources[2]?.blocks.map((block) => block.name)).toContain('mediaRender');
  });

  it('a registry with an unknown label key fails to parse', () => {
    expect(() =>
      parseResourcePageBlocksRegistry({
        resources: [{ key: 'items', labelKey: 'resourcePage.nope', default: [], blocks: [] }],
      }),
    ).toThrow();
  });

  it('drafts fall back to the theme default, and a stored map wins', () => {
    const fallback = resourcePageBlockDrafts(RESOURCE_PAGE_BLOCKS_REGISTRY, {});
    expect(fallback['items']).toContain('mediaList');
    expect(fallback['media']).toEqual(['mediaRender', 'values']);

    const stored = resourcePageBlockDrafts(RESOURCE_PAGE_BLOCKS_REGISTRY, {
      resource_page_blocks: { items: { main: ['values'] } },
    });
    expect(stored['items']).toEqual(['values']);
    // The stored map is the administrator's whole answer, so an omitted
    // resource is empty rather than the theme's default.
    expect(stored['item_sets']).toEqual([]);
    expect(stored['media']).toEqual([]);
  });

  it('the wire map names every resource', () => {
    const wire = resourcePageBlocksWire(RESOURCE_PAGE_BLOCKS_REGISTRY, {
      items: ['values'],
      item_sets: [],
      media: ['mediaRender'],
    });
    expect(wire['items']).toEqual({ main: ['values'] });
    expect(wire['item_sets']).toEqual({ main: [] });
    expect(wire['media']).toEqual({ main: ['mediaRender'] });
  });
});

describe('the contrast checks', () => {
  it('parses hex colours and computes the WCAG ratio', () => {
    expect(parseHex('#fff')).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseHex('#zzz')).toBeNull();
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 1);
  });

  it('the default palette passes all three pairs', () => {
    const checks = contrastChecks({
      text: '#222222',
      background: '#ffffff',
      link: '#1a5d8f',
      brand: '#6aa9d6',
    });
    expect(checks.map((check) => check.key)).toEqual([
      'text-background',
      'link-background',
      'text-brand',
    ]);
    expect(checks.every((check) => check.passes)).toBe(true);
  });

  it('a low-contrast pair fails, and a non-hex value has no ratio', () => {
    const checks = contrastChecks({
      text: '#eeeeee',
      background: '#ffffff',
      link: '#1a5d8f',
      brand: '#6aa9d6',
    });
    expect(checks[0]?.passes).toBe(false);
    expect(checks[0]?.ratio).not.toBeNull();
    const invalid = contrastChecks({
      text: 'red',
      background: '#ffffff',
      link: '#1a5d8f',
      brand: '#6aa9d6',
    });
    expect(invalid[0]?.ratio).toBeNull();
    expect(invalid[0]?.passes).toBe(false);
  });
});

const site = (): unknown => loadFixture('site-admin.json');

/** The preview route answers an HTML document, not JSON. */
const htmlResponse = (body: string): Response =>
  new Response(body, { status: 200, headers: { 'Content-Type': 'text/html; charset=utf-8' } });

const themeHandler = (url: URL): Response => {
  if (url.pathname === '/api/sites/1') {
    return jsonResponse(site());
  }
  if (url.pathname === '/api/site_pages/1') {
    return jsonResponse(loadFixture('site-page-1.json'));
  }
  if (url.pathname.endsWith('/preview')) {
    return htmlResponse('<html><body>preview</body></html>');
  }
  return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
};

describe('the site Theme page', () => {
  it('patches the token map without touching the site settings', async () => {
    const fetchMock = installFetch(themeHandler);
    const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);

    await waitFor(() => {
      expect(container.querySelector('#theme-setting-brand_color')).not.toBeNull();
    });
    const brand = container.querySelector('#theme-setting-brand_color');
    if (brand !== null) {
      fireEvent.change(brand, { target: { value: '#abcdef' } });
    }
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter((request) => request.init?.method === 'PATCH'),
    );
    const body = requestBody(patch);
    expect(body).not.toHaveProperty('o:site_settings');
    const tokens = body['o:theme_settings'];
    expect(tokens).toMatchObject({ brand_color: '#abcdef', color_scheme: 'light' });
    // The resource page block map travels in the same member, so a token save
    // does not drop it. The site stores no override, so the theme's set is
    // written.
    expect(tokens).toMatchObject({
      resource_page_blocks: {
        items: {
          main: ['mediaEmbeds', 'values', 'itemSets', 'sitePages', 'mediaList', 'linkedResources'],
        },
        media: { main: ['mediaRender', 'values'] },
      },
    });
  });

  it('blocks saving a palette below AA 4.5:1', async () => {
    installFetch(themeHandler);
    const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);

    await waitFor(() => {
      expect(container.querySelector('#theme-setting-text_color')).not.toBeNull();
    });
    const text = container.querySelector('#theme-setting-text_color');
    if (text !== null) {
      fireEvent.change(text, { target: { value: '#eeeeee' } });
    }

    expect(await screen.findByText('Fix the contrast before saving.')).toBeInTheDocument();
    const row = container.querySelector('[data-contrast="text-background"]');
    expect(row?.getAttribute('data-pass')).toBe('false');
    expect(container.querySelector('[data-contrast="link-background"]')?.getAttribute('data-pass')).toBe('true');
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  });

  it('posts the unsaved tokens to the preview route', async () => {
    const fetchMock = installFetch(themeHandler);
    render(<SitesPage param="1" rest={['1', 'theme']} />);

    await waitFor(
      () => {
        expect(requestsFor(fetchMock, '/s/my-site/preview')).not.toHaveLength(0);
      },
      { timeout: 3000 },
    );
    const preview = first(requestsFor(fetchMock, '/s/my-site/preview'));
    expect(preview.init?.method).toBe('POST');
    const body = requestBody(preview);
    expect(body['o:id']).toBe(1);
    expect(body['o:theme_settings']).toMatchObject({ brand_color: '#6aa9d6' });
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});
