/**
 * Colour arithmetic for the Theme page: hex parsing, OKLab and WCAG contrast.
 *
 * This is `crates/omeka-site/src/theme/color.rs` line for line, so the swatches
 * and the contrast gate react on every keystroke without a server round. The
 * two must give the same hex for the same input: `tests/theme-colour.test.ts`
 * replays the samples in `src/generated/themeSamples.json`, which a Rust test
 * writes from the Rust side.
 */

/** The WCAG AA ratio for body text. */
export const AA = 4.5;

/** The step the lightness searches take, in OKLab `L`. */
const STEP = 0.005;

/** A colour as three 0 to 255 channels. */
export type Rgb = readonly [number, number, number];

/** A colour in OKLab. */
export interface Oklab {
  readonly l: number;
  readonly a: number;
  readonly b: number;
}

const HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;

/** Whether a value is a hex colour a token accepts, as `HexColor::parse` reads it. */
export const isHexColour = (value: unknown): value is string =>
  typeof value === 'string' && HEX.test(value.trim());

/** `#rgb` or `#rrggbb` into channels, `null` for anything else. */
export const parseHex = (raw: string): Rgb | null => {
  const value = raw.trim();
  if (!HEX.test(value)) {
    return null;
  }
  const hex = value.slice(1);
  const full = hex.length === 3 ? hex.replace(/./g, (ch) => ch + ch) : hex;
  return [
    Number.parseInt(full.slice(0, 2), 16),
    Number.parseInt(full.slice(2, 4), 16),
    Number.parseInt(full.slice(4, 6), 16),
  ];
};

/** The normalised `#rrggbb` form of a hex colour, or `null`. */
export const normaliseHex = (raw: string): string | null => {
  const rgb = parseHex(raw);
  return rgb === null ? null : toHexString(rgb);
};

const toHexString = (rgb: Rgb): string =>
  `#${rgb.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;

const linear = (channel: number): number =>
  channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;

const gamma = (channel: number): number =>
  channel <= 0.0031308 ? channel * 12.92 : 1.055 * channel ** (1 / 2.4) - 0.055;

const linearChannels = (rgb: Rgb): [number, number, number] => [
  linear(rgb[0] / 255),
  linear(rgb[1] / 255),
  linear(rgb[2] / 255),
];

/** A hex colour in OKLab. */
export const toOklab = (hex: string): Oklab => {
  const rgb = parseHex(hex) ?? [0, 0, 0];
  const [r, g, b] = linearChannels(rgb);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return {
    l: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
};

const linearRgb = (l: number, a: number, b: number): [number, number, number] => {
  const l1 = l + 0.3963377774 * a + 0.2158037573 * b;
  const m1 = l - 0.1055613458 * a - 0.0638541728 * b;
  const s1 = l - 0.0894841775 * a - 1.291485548 * b;
  const l3 = l1 * l1 * l1;
  const m3 = m1 * m1 * m1;
  const s3 = s1 * s1 * s1;
  return [
    4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
    -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
    -0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3,
  ];
};

const inGamut = (rgb: readonly number[]): boolean =>
  rgb.every((channel) => channel >= -1e-7 && channel <= 1.0000001);

const clamp = (value: number, low: number, high: number): number =>
  Math.min(high, Math.max(low, value));

/** The chroma of an OKLab colour, its distance from grey. */
export const chroma = (lab: Oklab): number => Math.sqrt(lab.a * lab.a + lab.b * lab.b);

/** The nearest sRGB hex: chroma shrinks until the colour fits, keeping lightness and hue. */
export const oklabToHex = (lab: Oklab): string => {
  const l = clamp(lab.l, 0, 1);
  const fits = (scale: number): boolean => inGamut(linearRgb(l, lab.a * scale, lab.b * scale));
  let scale = 1;
  if (!fits(1)) {
    let low = 0;
    let high = 1;
    for (let step = 0; step < 24; step += 1) {
      const middle = (low + high) / 2;
      if (fits(middle)) {
        low = middle;
      } else {
        high = middle;
      }
    }
    scale = low;
  }
  const rgb = linearRgb(l, lab.a * scale, lab.b * scale);
  return toHexString([
    Math.round(gamma(clamp(rgb[0], 0, 1)) * 255),
    Math.round(gamma(clamp(rgb[1], 0, 1)) * 255),
    Math.round(gamma(clamp(rgb[2], 0, 1)) * 255),
  ]);
};

/** WCAG 2 relative luminance of a hex colour. */
export const luminance = (hex: string): number => {
  const [r, g, b] = linearChannels(parseHex(hex) ?? [0, 0, 0]);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** The WCAG 2 contrast ratio of two hex colours, 1 to 21. */
export const contrast = (first: string, second: string): number => {
  const x = luminance(first);
  const y = luminance(second);
  const [lighter, darker] = x >= y ? [x, y] : [y, x];
  return (lighter + 0.05) / (darker + 0.05);
};

/** Whether a colour reaches `ratio` against every colour in `against`. */
export const passes = (colour: string, against: readonly string[], ratio: number): boolean =>
  against.every((other) => contrast(colour, other) >= ratio);

const extreme = (against: readonly string[]): string => {
  const worst = (colour: string): number =>
    against.reduce((low, other) => Math.min(low, contrast(colour, other)), Infinity);
  return worst('#ffffff') >= worst('#000000') ? '#ffffff' : '#000000';
};

/**
 * The colour on `colour`'s hue and chroma whose lightness is nearest its own
 * and reaches `ratio` against every colour in `against`. At an equal distance
 * the darker candidate wins.
 */
export const nearestPassing = (
  colour: string,
  against: readonly string[],
  ratio: number,
): string => {
  const own = normaliseHex(colour) ?? '#000000';
  if (passes(own, against, ratio)) {
    return own;
  }
  const lab = toOklab(own);
  for (let index = 1; index <= 200; index += 1) {
    const offset = STEP * index;
    for (const l of [lab.l - offset, lab.l + offset]) {
      if (l >= 0 && l <= 1) {
        const candidate = oklabToHex({ ...lab, l });
        if (passes(candidate, against, ratio)) {
          return candidate;
        }
      }
    }
  }
  return extreme(against);
};

/** Step `start`'s lightness in `direction` until it reaches `ratio` against `against`. */
export const stepUntil = (
  start: Oklab,
  direction: number,
  against: readonly string[],
  ratio: number,
): string => {
  for (let index = 0; index <= 200; index += 1) {
    const l = clamp(start.l + direction * STEP * index, 0, 1);
    const candidate = oklabToHex({ ...start, l });
    if (passes(candidate, against, ratio)) {
      return candidate;
    }
    if (l <= 0 || l >= 1) {
      break;
    }
  }
  return extreme(against);
};
