import { ApiError } from '../api/client';
import type { Locale, Translator } from '../i18n';

const SIZE_UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;
const sizeFormats = new Map<string, Intl.NumberFormat>();

/**
 * A size in the locale's unit words. The steps are 1000, as the unit words
 * mean them: 1 GB is 1,000,000,000 bytes, the number a limit setting holds.
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

/** The upstream message with its status suffix, through the catalogue. */
export const formatError = (error: Error, translator: Translator): string =>
  error instanceof ApiError
    ? `${error.message}${translator.t('format.httpSuffix', { status: error.status })}`
    : error.message;

// One formatter per locale: building an `Intl.DateTimeFormat` costs far more
// than formatting with one, and a list formats a date in every row.
const dateTimeFormats = new Map<Locale, Intl.DateTimeFormat>();

const dateTimeFormat = (locale: Locale): Intl.DateTimeFormat => {
  let format = dateTimeFormats.get(locale);
  if (format === undefined) {
    // Upstream's time zone is the installation's `time_zone` setting, whose
    // default is UTC; the API sends no setting the admin could read instead.
    format = new Intl.DateTimeFormat(locale, {
      dateStyle: 'medium',
      timeStyle: 'short',
      timeZone: 'UTC',
    });
    dateTimeFormats.set(locale, format);
  }
  return format;
};

/**
 * An ISO 8601 date as upstream's `i18n()->dateFormat($date, 'medium', 'short')`
 * prints it (`View/Helper/I18n.php`): a medium date and a short time in the
 * locale. A value that does not parse is returned as it arrived.
 */
export const formatDateTime = (iso: string, locale: Locale): string => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return dateTimeFormat(locale).format(date);
};

/** `MediaController::showAction` prints a media size as `%s bytes`. */
export const formatBytes = (size: number, translator: Translator): string =>
  translator.t('format.bytes', { size: translator.number(size) });
