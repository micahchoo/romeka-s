/**
 * A size in the locale's unit words, for the upload rows and the limits line.
 * Only the import and the add-media panel read it, so it stays out of the
 * eager `app/format.ts` the whole admin carries on its first paint.
 */
import type { Locale } from '../../i18n';

const SIZE_UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;
const sizeFormats = new Map<string, Intl.NumberFormat>();

/**
 * The steps are 1000, as the unit words mean them: 1 GB is 1,000,000,000
 * bytes, the number a limit setting holds.
 */
export const sizeText = (bytes: number, locale: Locale): string => {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < SIZE_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const name = SIZE_UNITS[unit] ?? 'byte';
  const key = `${locale}:${name}`;
  let format = sizeFormats.get(key);
  if (format === undefined) {
    format = new Intl.NumberFormat(locale, { style: 'unit', unit: name, unitDisplay: unit === 0 ? 'long' : 'short', maximumFractionDigits: unit < 2 ? 0 : 1 });
    sizeFormats.set(key, format);
  }
  return format.format(value);
};
