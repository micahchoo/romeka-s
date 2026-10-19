/**
 * `formatDateTime`, the admin's date and time. Upstream prints a date through
 * `IntlDateFormatter` with a medium date and a short time in the active locale
 * (`application/src/View/Helper/I18n.php`), in the installation time zone,
 * which defaults to UTC.
 */
import { describe, expect, it } from 'vitest';
import { formatDateTime } from '../src/app/format';
import { LOCALES } from '../src/i18n';

const ISO = '2024-02-10T08:15:00+00:00';
// ICU puts a narrow no-break space before the day period; a reader sees a space.
const plain = (text: string): string => text.replace(/\s/g, ' ');

describe('formatDateTime', () => {
  it('prints_a_medium_date_and_a_short_time_in_english', () => {
    expect(plain(formatDateTime(ISO, 'en'))).toBe('Feb 10, 2024, 8:15 AM');
  });

  it('prints_the_spanish_date_order_and_month', () => {
    expect(plain(formatDateTime(ISO, 'es'))).toBe('10 feb 2024, 8:15');
  });

  it('prints_every_shipped_locale_in_its_own_form', () => {
    const outputs = LOCALES.map((locale) => formatDateTime(ISO, locale));
    expect(outputs).not.toContain(ISO);
    // A locale the runtime has no data for falls back to English and repeats it.
    expect(new Set(outputs).size).toBe(LOCALES.length);
  });

  it('returns_an_unparseable_value_as_it_arrived', () => {
    expect(formatDateTime('not a date', 'en')).toBe('not a date');
  });
});
