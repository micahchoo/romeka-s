/**
 * The site Theme page and its pure pieces: the draft a stored map opens as,
 * the map a save writes, the contrast gate with its suggestions, the theme
 * file, and the page that saves and previews a theme.
 *
 * The page runs through the real typed client against a stubbed fetch, so the
 * body it PATCHes and the body it POSTs to the preview are the bytes the server
 * would get.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  parseResourcePageBlocksRegistry,
  resourcePageBlockDrafts,
  resourcePageBlocksWire,
} from '../src/pages/sites/resourcePageBlocksSchema';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from '../src/pages/sites/resourcePageBlocksRegistry';
import { AA, contrast } from '../src/pages/sites/themeEditor/colour';
import {
  contrastChecks,
  draftFromStored,
  draftWire,
  exportTheme,
  importTheme,
  resolve,
  setOverride,
} from '../src/pages/sites/themeEditor/draft';
import { parseThemeRegistry, THEME_REGISTRY } from '../src/pages/sites/themeEditor/registry';
import { setLocale } from '../src/i18n';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';

// The Hindi Theme table arrives only when a test opens the gate, so a locale
// switch's wait is observable: the catalogue has loaded and the table has not.
const hindiTheme = vi.hoisted(() => {
  let open: () => void = () => undefined;
  const gate = new Promise<void>((resolve) => {
    open = resolve;
  });
  return {
    gate,
    open: () => {
      open();
    },
  };
});
vi.mock('../src/i18n/namespaces/theme/locales/hi', async (importOriginal) => {
  await hindiTheme.gate;
  return importOriginal();
});
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';

describe('the theme draft', () => {
  it('the generated registry lists the presets, the pairs and every token', () => {
    expect(THEME_REGISTRY.presets.map((preset) => preset.id)).toEqual([
      'plain',
      'archive',
      'gallery',
      'editorial',
      'civic',
    ]);
    expect(THEME_REGISTRY.pairs).toContainEqual(['on-brand', 'brand']);
    expect(THEME_REGISTRY.settings.map((setting) => setting.key)).toContain('custom_css');
  });

  it('a registry with an unknown catalogue key fails to parse', () => {
    expect(() =>
      parseThemeRegistry({
        newSitePreset: 'plain',
        settings: [{ key: 'x', labelKey: 'themeSetting.nope', helpKey: 'theme.reset', kind: 'text' }],
        presets: [],
        colours: [],
        pairs: [],
        aa: 4.5,
      }),
    ).toThrow();
  });

  it('a map from before presets opens as plain, derivation off, with only its real changes', () => {
    const draft = draftFromStored({
      brand_color: '#6aa9d6',
      text_color: '#111111',
      heading_font: 'system-serif',
      base_font_size: 16,
      logo: 7,
    });
    expect(draft.preset).toBe('plain');
    expect(draft.derive).toBe(false);
    expect(draft.overrides).toEqual({ text_color: '#111111' });
    expect(draft.content.logo).toBe('7');
  });

  it('a stored preset derives by default and keeps a refused value out', () => {
    const draft = draftFromStored({ preset: 'civic', base_font_size: 400, link_color: 'red' });
    expect(draft.derive).toBe(true);
    expect(draft.overrides).toEqual({});
  });

  it('the wire map carries the design, the content and trimmed custom CSS', () => {
    let draft = draftFromStored({ preset: 'archive', banner: 3 });
    draft = setOverride(draft, 'base_font_size', 19);
    draft = { ...draft, customCss: '  a { color: red }\n', content: { ...draft.content, footer_text: ' ' } };
    expect(draftWire(draft)).toEqual({
      preset: 'archive',
      derive_palette: true,
      base_font_size: 19,
      custom_css: 'a { color: red }',
      logo: null,
      banner: 3,
      banner_text: null,
      footer_text: null,
    });
  });

  it('a failing pair offers the nearest colour that passes on every background it sits on', () => {
    const draft = setOverride(draftFromStored({ preset: 'plain' }), 'muted_color', '#bbbbbb');
    const checks = contrastChecks(resolve(draft));
    const muted = checks.filter((check) => check.foreground === 'muted' && !check.passes);
    expect(muted.length).toBeGreaterThan(0);
    const suggestion = muted[0]?.suggestion ?? '';
    const palette = resolve(setOverride(draft, 'muted_color', suggestion)).light;
    expect(contrast(suggestion, palette.background)).toBeGreaterThanOrEqual(AA);
    expect(contrast(suggestion, palette.surface)).toBeGreaterThanOrEqual(AA);
  });

  it('the automatic scheme checks both palettes', () => {
    const checks = contrastChecks(resolve(setOverride(draftFromStored({ preset: 'civic' }), 'color_scheme', 'auto')));
    expect(new Set(checks.map((check) => check.shade))).toEqual(new Set(['light', 'dark']));
    expect(checks.every((check) => check.passes)).toBe(true);
  });

  it('a theme file round-trips the design and keeps the site content', () => {
    const source = setOverride(draftFromStored({ preset: 'gallery' }), 'brand_color', '#ff6600');
    const target = draftFromStored({ preset: 'plain', logo: 9 });
    const imported = importTheme(exportTheme({ ...source, customCss: 'p { margin: 0 }' }), target);
    expect(imported?.preset).toBe('gallery');
    expect(imported?.overrides).toEqual({ brand_color: '#ff6600' });
    expect(imported?.customCss).toBe('p { margin: 0 }');
    expect(imported?.content.logo).toBe('9');
    expect(importTheme('{"preset": "gallery"}', target)).toBeNull();
    expect(importTheme('not json', target)).toBeNull();
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
  it('applies a preset in one click and saves it with the resource page blocks', async () => {
    const fetchMock = installFetch(themeHandler);
    render(<SitesPage param="1" rest={['1', 'theme']} />);

    fireEvent.click(await screen.findByRole('button', { name: /^Civic/ }));
    expect(screen.getByRole('button', { name: /^Civic/ })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter((request) => request.init?.method === 'PATCH'),
    );
    const body = requestBody(patch);
    expect(body).not.toHaveProperty('o:site_settings');
    const settings = body['o:theme_settings'];
    // The fixture's map predates presets, so derivation starts off, and a
    // preset keeps the switch where the curator left it.
    expect(settings).toMatchObject({ preset: 'civic', derive_palette: false, logo: 7 });
    expect(settings).not.toHaveProperty('brand_color');
    expect(settings).toMatchObject({
      resource_page_blocks: {
        items: {
          main: ['mediaEmbeds', 'values', 'itemSets', 'sitePages', 'mediaList', 'linkedResources'],
        },
        media: { main: ['mediaRender', 'values'] },
      },
    });
  });

  it('a switch to Hindi waits for the Theme table and keeps the open page and its draft', async () => {
    installFetch(themeHandler);
    const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);
    expect(await screen.findByRole('tab', { name: 'Design' })).toBeInTheDocument();
    await waitFor(() => {
      expect(container.querySelector('#theme-setting-text_color')).not.toBeNull();
    });
    const text = container.querySelector('#theme-setting-text_color');
    if (text !== null) {
      fireEvent.change(text, { target: { value: '#123456' } });
    }
    try {
      const switching = setLocale('hi');
      // While the Hindi table is held back the page stays English and mounted.
      await expect(screen.findByRole('tab', { name: 'डिज़ाइन' }, { timeout: 300 })).rejects.toThrow();
      expect(screen.getByRole('tab', { name: 'Design' })).toBeInTheDocument();

      hindiTheme.open();
      await act(() => switching);
      expect(await screen.findByRole('tab', { name: 'डिज़ाइन' })).toBeInTheDocument();
      expect(container.querySelector('#theme-setting-text_color')).toHaveValue('#123456');
    } finally {
      await act(() => setLocale('en'));
    }
  });

  it('blocks a palette below AA and applies the suggested colour', async () => {
    installFetch(themeHandler);
    const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);

    await waitFor(() => {
      expect(container.querySelector('#theme-setting-text_color')).not.toBeNull();
    });
    const text = container.querySelector('#theme-setting-text_color');
    if (text !== null) {
      fireEvent.change(text, { target: { value: '#eeeeee' } });
    }

    expect(await screen.findByText('Fix the problems above before saving.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    const row = container.querySelector('[data-contrast="text-background"]');
    expect(row).not.toBeNull();
    if (row instanceof HTMLElement) {
      fireEvent.click(within(row).getByRole('button', { name: /^Use #[0-9a-f]{6} for Text$/ }));
    }
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
    });
    expect(screen.getByText(/Every text and background pair passes/)).toBeInTheDocument();
  });

  it('a fine-tuned value shows Reset, and Reset returns it to the preset', async () => {
    const fetchMock = installFetch(themeHandler);
    const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);

    await waitFor(() => {
      expect(container.querySelector('#theme-setting-content_width')).not.toBeNull();
    });
    const width = container.querySelector('#theme-setting-content_width');
    if (width !== null) {
      fireEvent.change(width, { target: { value: 'narrow' } });
    }
    const reset = screen.getByRole('button', { name: 'Reset Content width to the preset value' });
    fireEvent.click(reset);
    expect(screen.queryByRole('button', { name: 'Reset Content width to the preset value' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter((request) => request.init?.method === 'PATCH'),
    );
    expect(requestBody(patch)['o:theme_settings']).not.toHaveProperty('content_width');
  });

  it('posts the unsaved theme to the preview route', async () => {
    const fetchMock = installFetch(themeHandler);
    const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);
    await waitFor(() => {
      expect(container.querySelector('#theme-setting-brand_color')).not.toBeNull();
    });
    const brand = container.querySelector('#theme-setting-brand_color');
    if (brand !== null) {
      fireEvent.change(brand, { target: { value: '#abcdef' } });
    }

    await waitFor(
      () => {
        const bodies = requestsFor(fetchMock, '/s/my-site/preview').map(requestBody);
        expect(bodies.some((body) => isBrand(body['o:theme_settings'], '#abcdef'))).toBe(true);
      },
      { timeout: 3000 },
    );
  });

  it('shows the sanitiser errors from the preview and blocks the save', async () => {
    installFetch((url, init) => {
      if (url.pathname.endsWith('/preview') && typeof init?.body === 'string' && init.body.includes('@import')) {
        return jsonResponse(
          { errors: { 'o:theme_settings': ['Custom CSS: Line 1, column 1: @import is not allowed.'] } },
          { status: 422 },
        );
      }
      return themeHandler(url);
    });
    render(<SitesPage param="1" rest={['1', 'theme']} />);

    fireEvent.click(await screen.findByRole('tab', { name: 'Custom CSS' }));
    const editor = await screen.findByRole('textbox', { name: 'Custom CSS' });
    fireEvent.change(editor, { target: { value: "@import 'x.css';" } });

    expect(
      await screen.findByRole(
        'button',
        { name: 'Custom CSS: Line 1, column 1: @import is not allowed.' },
        { timeout: 3000 },
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  });

  it('shows a refused @import in spanish and still marks its line', async () => {
    await setLocale('es');
    installFetch((url, init) => {
      if (url.pathname.endsWith('/preview') && typeof init?.body === 'string' && init.body.includes('@import')) {
        return jsonResponse(
          { errors: { 'o:theme_settings': ['Custom CSS: Line 2, column 1: @import is not allowed.'] } },
          { status: 422 },
        );
      }
      return themeHandler(url);
    });
    try {
      const { container } = render(<SitesPage param="1" rest={['1', 'theme']} />);

      fireEvent.click(await screen.findByRole('tab', { name: 'CSS propio' }));
      const editor = await screen.findByRole('textbox', { name: 'CSS propio' });
      fireEvent.change(editor, { target: { value: "a { color: red }\n@import 'x.css';" } });

      const refusal = await screen.findByRole(
        'button',
        { name: 'CSS propio: línea 2, columna 1: @import no está permitido.' },
        { timeout: 3000 },
      );
      const marked = [...container.querySelectorAll('.css-editor-gutter [data-bad="true"]')];
      expect(marked.map((line) => line.textContent)).toEqual(['2']);
      fireEvent.click(refusal);
      expect(editor).toHaveFocus();
      expect(editor).toBeInstanceOf(HTMLTextAreaElement);
      expect(editor instanceof HTMLTextAreaElement ? editor.selectionStart : null).toBe('a { color: red }\n'.length);
    } finally {
      await setLocale('en');
    }
  });
});

const isBrand = (settings: unknown, brand: string): boolean =>
  typeof settings === 'object' && settings !== null && 'brand_color' in settings && settings.brand_color === brand;

afterEach(() => {
  vi.restoreAllMocks();
});
