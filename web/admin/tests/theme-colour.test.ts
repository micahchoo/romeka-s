/**
 * The admin's palette arithmetic gives the hex the Rust theme gives.
 *
 * `src/generated/themeSamples.json` is written by a Rust test from
 * `crates/omeka-site/src/theme/tokens_schema.rs::samples`: brand colours with
 * the palettes each preset derives, colours with their nearest passing
 * suggestion, and stored maps with the palettes they resolve to. Replaying
 * them here means the swatches and the contrast gate on the Theme page show
 * what the public page will render.
 */
import { describe, expect, it } from 'vitest';
import samples from '../src/generated/themeSamples.json';
import { AA, contrast, nearestPassing } from '../src/pages/sites/themeEditor/colour';
import { derive, onBrand } from '../src/pages/sites/themeEditor/palette';
import type { Palette } from '../src/pages/sites/themeEditor/palette';
import { draftFromStored, resolve } from '../src/pages/sites/themeEditor/draft';
import { presetFor } from '../src/pages/sites/themeEditor/registry';

const withOnBrand = (palette: Palette): Record<string, string> => ({
  brand: palette.brand,
  'on-brand': onBrand(palette),
  accent: palette.accent,
  background: palette.background,
  surface: palette.surface,
  text: palette.text,
  muted: palette.muted,
  link: palette.link,
});

describe('the palette arithmetic matches the Rust theme', () => {
  it('derives the same palettes from every sample brand', () => {
    expect(samples.derive.length).toBeGreaterThan(50);
    for (const sample of samples.derive) {
      const recipe = presetFor(sample.preset).recipe;
      expect(withOnBrand(derive(sample.brand, recipe, 'light')), `${sample.brand} ${sample.preset}`).toEqual(
        sample.light,
      );
      expect(withOnBrand(derive(sample.brand, recipe, 'dark')), `${sample.brand} ${sample.preset}`).toEqual(
        sample.dark,
      );
    }
  });

  it('suggests the same nearest passing colour', () => {
    for (const sample of samples.nearest) {
      expect(nearestPassing(sample.colour, sample.against, AA), sample.colour).toBe(sample.suggestion);
    }
  });

  it('resolves the same stored maps to the same palettes', () => {
    for (const sample of samples.resolve) {
      const resolved = resolve(draftFromStored(sample.stored));
      expect(resolved.scheme).toBe(sample.scheme);
      expect(withOnBrand(resolved.light), JSON.stringify(sample.stored)).toEqual(sample.light);
      expect(withOnBrand(resolved.dark), JSON.stringify(sample.stored)).toEqual(sample.dark);
    }
  });

  it('a suggestion passes and the WCAG reference points hold', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 6);
    expect(contrast('#767676', '#ffffff')).toBeCloseTo(4.54, 2);
    const suggestion = nearestPassing('#6aa9d6', ['#ffffff'], AA);
    expect(contrast(suggestion, '#ffffff')).toBeGreaterThanOrEqual(AA);
  });
});
