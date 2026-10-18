/**
 * WCAG contrast for the theme page. The readout checks the three pairs a
 * palette most often gets wrong, and the Save button refuses a palette whose
 * ratio is below AA 4.5:1.
 *
 * The formulas are WCAG 2.1 "relative luminance" and "contrast ratio". A value
 * that is not a hex colour has no ratio, so its check does not pass and does
 * not pretend to.
 */
import type { MessageKey } from '../../i18n';

/** The AA ratio for normal text. */
export const AA_CONTRAST = 4.5;

/** The sRGB channels a contrast check needs. */
export interface Rgb {
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

/** Parse `#rgb` or `#rrggbb` into channels, `null` for anything else. */
export const parseHex = (value: string): Rgb | null => {
  const hex = value.trim().replace(/^#/, '');
  if (!/^[0-9a-fA-F]+$/.test(hex)) {
    return null;
  }
  const expanded =
    hex.length === 3
      ? hex
          .split('')
          .map((channel) => channel + channel)
          .join('')
      : hex;
  if (expanded.length !== 6) {
    return null;
  }
  return {
    r: Number.parseInt(expanded.slice(0, 2), 16),
    g: Number.parseInt(expanded.slice(2, 4), 16),
    b: Number.parseInt(expanded.slice(4, 6), 16),
  };
};

const channel = (value: number): number => {
  const scaled = value / 255;
  return scaled <= 0.03928 ? scaled / 12.92 : ((scaled + 0.055) / 1.055) ** 2.4;
};

/** WCAG relative luminance. */
export const relativeLuminance = (rgb: Rgb): number =>
  0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b);

/** The contrast ratio between two colours, `null` when either is not a hex. */
export const contrastRatio = (foreground: string, background: string): number | null => {
  const fg = parseHex(foreground);
  const bg = parseHex(background);
  if (fg === null || bg === null) {
    return null;
  }
  const first = relativeLuminance(fg);
  const second = relativeLuminance(bg);
  const [lighter, darker] = first >= second ? [first, second] : [second, first];
  return (lighter + 0.05) / (darker + 0.05);
};

/** One pair the theme page watches. */
export interface ContrastCheck {
  readonly key: string;
  readonly labelKey: MessageKey;
  readonly foreground: string;
  readonly background: string;
  /** The ratio, `null` when a value is not a hex colour. */
  readonly ratio: number | null;
  readonly passes: boolean;
}

/** The colours the checks read. */
export interface ContrastPalette {
  readonly text: string;
  readonly background: string;
  readonly link: string;
  readonly brand: string;
}

/** The three AA checks, in the order the page shows them. */
export const contrastChecks = (palette: ContrastPalette): readonly ContrastCheck[] => {
  const pairs: readonly {
    key: string;
    labelKey: MessageKey;
    foreground: string;
    background: string;
  }[] = [
    {
      key: 'text-background',
      labelKey: 'theme.contrastTextBackground',
      foreground: palette.text,
      background: palette.background,
    },
    {
      key: 'link-background',
      labelKey: 'theme.contrastLinkBackground',
      foreground: palette.link,
      background: palette.background,
    },
    {
      key: 'text-brand',
      labelKey: 'theme.contrastTextBrand',
      foreground: palette.text,
      background: palette.brand,
    },
  ];
  return pairs.map((pair) => {
    const ratio = contrastRatio(pair.foreground, pair.background);
    return { ...pair, ratio, passes: ratio !== null && ratio >= AA_CONTRAST };
  });
};

/** A ratio as `4.52:1`, or a dash when there is none. */
export const formatRatio = (ratio: number | null): string =>
  ratio === null ? '—' : `${ratio.toFixed(2)}:1`;
