/**
 * A colour's dark value under the automatic scheme: the stored
 * `<key>_dark` member, its Fine-tune control beside the light one, and the
 * contrast gate that checks both palettes.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { contrastChecks, draftFromStored, draftWire, resolve } from '../src/pages/sites/themeEditor/draft';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';

describe('the dark values in the draft', () => {
  it('a stored dark value opens as its own override and is written back', () => {
    const draft = draftFromStored({
      preset: 'editorial',
      color_scheme: 'auto',
      muted_color: '#444444',
      muted_color_dark: '#BBBBBB',
    });
    expect(draft.overrides).toMatchObject({ muted_color: '#444444', muted_color_dark: '#bbbbbb' });
    expect(resolve(draft).dark.muted).toBe('#bbbbbb');
    expect(resolve(draft).light.muted).toBe('#444444');
    expect(draftWire(draft)).toMatchObject({ muted_color_dark: '#bbbbbb' });
  });

  it('a dark value that fails AA fails the gate under the automatic scheme', () => {
    const failing = contrastChecks(
      resolve(draftFromStored({ preset: 'editorial', color_scheme: 'auto', text_color_dark: '#222222' })),
    ).filter((check) => !check.passes);
    expect(failing.map((check) => [check.shade, check.foreground])).toContainEqual(['dark', 'text']);
    expect(failing.every((check) => check.shade === 'dark')).toBe(true);
  });
});

const siteWithTheme = (settings: Record<string, unknown>): Record<string, unknown> => {
  const raw = loadFixture('site-admin.json');
  if (!isRecord(raw)) {
    throw new Error('expected a site object');
  }
  return { ...raw, 'o:theme_settings': settings };
};

const renderTheme = (settings: Record<string, unknown>): ReturnType<typeof installFetch> => {
  const fetchMock = installFetch((url) => {
    if (url.pathname === '/api/sites/1') {
      return jsonResponse(siteWithTheme(settings));
    }
    if (url.pathname === '/api/site_pages/1') {
      return jsonResponse(loadFixture('site-page-1.json'));
    }
    if (url.pathname.endsWith('/preview')) {
      return new Response('<html><body>preview</body></html>', {
        headers: { 'Content-Type': 'text/html; charset=utf-8' },
      });
    }
    return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
  });
  render(<SitesPage param="1" rest={['1', 'theme']} />);
  return fetchMock;
};

describe('the Fine-tune colours under the automatic scheme', () => {
  it('each colour row shows a light and a dark value, and a dark edit saves its own key', async () => {
    const fetchMock = renderTheme({ preset: 'editorial', color_scheme: 'auto' });

    const dark = await screen.findByRole('textbox', { name: 'Muted text (Dark)' });
    expect(screen.getByRole('textbox', { name: 'Muted text (Light)' })).toBeInTheDocument();
    fireEvent.change(dark, { target: { value: '#bbbbbb' } });
    expect(screen.getByRole('textbox', { name: 'Muted text (Light)' })).not.toHaveValue('#bbbbbb');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter((request) => request.init?.method === 'PATCH'),
    );
    const stored = requestBody(patch)['o:theme_settings'];
    expect(stored).toMatchObject({ muted_color_dark: '#bbbbbb' });
    expect(stored).not.toHaveProperty('muted_color');
  });

  it('a dark value below AA blocks the save, and its suggestion fixes the dark value', async () => {
    renderTheme({ preset: 'editorial', color_scheme: 'auto' });

    fireEvent.change(await screen.findByRole('textbox', { name: 'Text (Dark)' }), {
      target: { value: '#222222' },
    });
    expect(await screen.findByText('Fix the problems above before saving.')).toBeInTheDocument();
    const row = document.querySelector('[data-contrast="text-background"]');
    if (!(row instanceof HTMLElement)) {
      throw new Error('the failing text pair is listed');
    }
    expect(row).toHaveTextContent('(Dark)');
    fireEvent.click(within(row).getByRole('button', { name: /^Use #[0-9a-f]{6} for Text$/ }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
    });
    expect(screen.getByRole('textbox', { name: 'Text (Dark)' })).not.toHaveValue('#222222');
  });

  it('a fixed scheme shows one value per colour', async () => {
    renderTheme({ preset: 'editorial', color_scheme: 'light' });

    expect(await screen.findByRole('textbox', { name: 'Muted text' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: 'Muted text (Dark)' })).toBeNull();
  });
});
