/**
 * When a page version went live, as the history panel and the conflict dialog
 * print it: the time alone, after "Today", for a save on the current UTC day,
 * else the medium date and short time every admin list prints. Both are UTC,
 * as `src/app/format.ts` explains.
 */
import { formatDateTime } from '../../../../app/format';
import type { Locale } from '../../../../i18n';

const timeFormats = new Map<Locale, Intl.DateTimeFormat>();

const timeFormat = (locale: Locale): Intl.DateTimeFormat => {
  let format = timeFormats.get(locale);
  if (format === undefined) {
    format = new Intl.DateTimeFormat(locale, { timeStyle: 'short', timeZone: 'UTC' });
    timeFormats.set(locale, format);
  }
  return format;
};

const utcDay = (date: Date): string => date.toISOString().slice(0, 10);

export const versionTime = (
  iso: string,
  locale: Locale,
  today: (time: string) => string,
  now: Date = new Date(),
): string => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return utcDay(date) === utcDay(now) ? today(timeFormat(locale).format(date)) : formatDateTime(iso, locale);
};
