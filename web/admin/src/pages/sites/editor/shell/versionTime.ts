/**
 * When a page version went live, as the history panel and the conflict dialog
 * print it: the time alone, after "Today", for a save on the current day, else
 * the medium date and short time every admin list prints. Both are in the
 * installation's time zone, as `src/app/format.ts` explains.
 */
import { formatDateTime, zonedFormat } from '../../../../app/format';
import type { Locale } from '../../../../i18n';

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
  const day = zonedFormat(locale, 'day');
  return day.format(date) === day.format(now)
    ? today(zonedFormat(locale, 'time').format(date))
    : formatDateTime(iso, locale);
};
