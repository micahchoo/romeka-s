import { ApiError } from '../api/client';
import type { Locale, Translator } from '../i18n';
import { installation } from './installation';

/** The upstream message with its status suffix, through the catalogue. */
export const formatError = (error: Error, translator: Translator): string =>
  error instanceof ApiError
    ? `${error.message}${translator.t('format.httpSuffix', { status: error.status })}`
    : error.message;

const STYLES = {
  /** `dateFormat($date, 'medium', 'short')`. */
  dateTime: { dateStyle: 'medium', timeStyle: 'short' },
  time: { timeStyle: 'short' },
  /** The calendar day, for "is this today" and nothing printed. */
  day: { year: 'numeric', month: '2-digit', day: '2-digit' },
} as const satisfies Record<string, Intl.DateTimeFormatOptions>;

export type DateStyle = keyof typeof STYLES;

// One formatter per locale, zone and style: building an `Intl.DateTimeFormat`
// costs far more than formatting with one, and a list formats a date in every row.
const formats = new Map<string, Intl.DateTimeFormat>();

/** A formatter in the installation's `time_zone`, the zone upstream's `I18nFactory` prints in. */
export const zonedFormat = (locale: Locale, style: DateStyle): Intl.DateTimeFormat => {
  const { timeZone } = installation();
  const key = `${locale} ${timeZone} ${style}`;
  let format = formats.get(key);
  if (format === undefined) {
    try {
      format = new Intl.DateTimeFormat(locale, { ...STYLES[style], timeZone });
    } catch {
      // A zone newer than this browser's time zone data: UTC, upstream's default.
      format = new Intl.DateTimeFormat(locale, { ...STYLES[style], timeZone: 'UTC' });
    }
    formats.set(key, format);
  }
  return format;
};

/**
 * An ISO 8601 date as upstream's `i18n()->dateFormat($date, 'medium', 'short')`
 * prints it (`View/Helper/I18n.php`): a medium date and a short time in the
 * locale, in the installation's `time_zone` (`I18nFactory`). A value that does
 * not parse is returned as it arrived.
 */
export const formatDateTime = (iso: string, locale: Locale): string => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return zonedFormat(locale, 'dateTime').format(date);
};

/** `MediaController::showAction` prints a media size as `%s bytes`. */
export const formatBytes = (size: number, translator: Translator): string =>
  translator.t('format.bytes', { size: translator.number(size) });
