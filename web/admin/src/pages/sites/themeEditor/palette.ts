/**
 * The theme's palette: the colour roles, text on the brand colour, the
 * derivation from one brand colour, and the contrast pairs.
 *
 * This is `crates/omeka-site/src/theme/palette.rs`, kept identical by the
 * samples `tests/theme-colour.test.ts` replays.
 */
import { AA, chroma, contrast, normaliseHex, oklabToHex, stepUntil, toOklab } from './colour';
import type { Oklab } from './colour';

/** A colour a curator may set. */
export type ColourRole = 'brand' | 'accent' | 'background' | 'surface' | 'text' | 'muted' | 'link';

/** A colour the sheet reads: the editable roles plus text on the brand colour. */
export type Role = ColourRole | 'on-brand';

export const COLOUR_ROLES: readonly ColourRole[] = [
  'brand',
  'accent',
  'background',
  'surface',
  'text',
  'muted',
  'link',
];

/** The seven colours a curator picks. */
export type Palette = Readonly<Record<ColourRole, string>>;

/** How a preset turns one brand colour into a palette. */
export interface Recipe {
  readonly tint: number;
  readonly paper: number;
  readonly ink: number;
}

export type Shade = 'light' | 'dark';

/**
 * Text on the brand colour: the palette's text or background colour,
 * whichever contrasts more, when it reaches AA; otherwise white or black.
 */
export const onBrand = (palette: Palette): string => {
  const { brand, text, background } = palette;
  const best = contrast(text, brand) >= contrast(background, brand) ? text : background;
  if (contrast(best, brand) >= AA) {
    return best;
  }
  return contrast('#ffffff', brand) >= contrast('#000000', brand) ? '#ffffff' : '#000000';
};

/** The colour a role resolves to. */
export const roleColour = (palette: Palette, role: Role): string =>
  role === 'on-brand' ? onBrand(palette) : palette[role];

/** The accent's hue when the brand is a grey and has none: a warm amber. */
const GREY_ACCENT_HUE: readonly [number, number] = [0.4186, 0.9082];

/** The palette for one shade, derived from `brand` the way `palette::derive` does. */
export const derive = (brand: string, recipe: Recipe, shade: Shade): Palette => {
  const lab = toOklab(brand);
  const c = chroma(lab);
  const grey = c <= 1e-4;
  const hueA = grey ? 0 : lab.a / c;
  const hueB = grey ? 0 : lab.b / c;
  const [accentA, accentB] = grey ? GREY_ACCENT_HUE : [-hueA, -hueB];
  const tinted = (l: number, amount: number): Oklab => ({ l, a: hueA * amount, b: hueB * amount });
  const linkChroma = Math.max(Math.min(Math.max(c, 0), 0.2), grey ? 0 : 0.08);
  const accent = (l: number): Oklab => ({ l, a: accentA * 0.12, b: accentB * 0.12 });
  if (shade === 'light') {
    const background = oklabToHex(tinted(recipe.paper, 0.012 * recipe.tint));
    const surface = oklabToHex(tinted(recipe.paper - 0.035, 0.02 * recipe.tint));
    const darker = (start: Oklab): string => stepUntil(start, -1, [background, surface], AA);
    return {
      brand: brandHex(brand),
      accent: darker(accent(0.56)),
      background,
      surface,
      text: darker(tinted(0.25, 0.02 * recipe.tint)),
      muted: darker(tinted(0.52, 0.03 * recipe.tint)),
      link: darker(tinted(Math.min(lab.l, 0.6), linkChroma)),
    };
  }
  const background = oklabToHex(tinted(recipe.ink, 0.012 * recipe.tint));
  const surface = oklabToHex(tinted(recipe.ink + 0.05, 0.018 * recipe.tint));
  const lighter = (start: Oklab): string => stepUntil(start, 1, [background, surface], AA);
  return {
    brand: brandHex(brand),
    accent: lighter(accent(0.8)),
    background,
    surface,
    text: lighter(tinted(0.94, 0.01 * recipe.tint)),
    muted: lighter(tinted(0.74, 0.025 * recipe.tint)),
    link: lighter(tinted(Math.max(lab.l, 0.78), linkChroma)),
  };
};

const brandHex = (brand: string): string => normaliseHex(brand) ?? brand;

/** One contrast pair, checked. */
export interface PairCheck {
  readonly shade: Shade;
  readonly foreground: Role;
  readonly background: Role;
  readonly ratio: number;
  readonly passes: boolean;
  /** The nearest foreground that passes, when this pair fails and the foreground is editable. */
  readonly suggestion: string | null;
}
