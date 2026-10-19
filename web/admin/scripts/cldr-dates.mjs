/**
 * Write the public site's CLDR date tables for the Indic locales.
 *
 *   node scripts/cldr-dates.mjs
 *
 * Upstream's `I18n::dateFormat` formats a date through PHP `intl`, which is ICU
 * and so CLDR, whenever `intl` is loaded (`application/src/View/Helper/I18n.php:93-140`).
 * The port has no ICU, so `crates/omeka-site/src/i18n/dates_cldr.rs` carries what
 * ICU would print: the month and weekday names in their format context, the
 * `dateStyle` and `timeStyle` patterns turned into the PHP date characters the
 * port's formatter reads, the day periods, and the digits of the locale's default
 * numbering system. Node's `Intl` is ICU, so it is the source, and the file names
 * the ICU and CLDR versions that produced it. The model never writes a date name.
 *
 * It runs under node, not bun: bun's `Intl` is not ICU.
 */
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const OUT = join(dirname(fileURLToPath(import.meta.url)), '../../../crates/omeka-site/src/i18n/dates_cldr.rs');
const LOCALES = ['hi', 'mr', 'ml', 'ta', 'kn', 'te', 'ne'];
const STYLES = ['short', 'medium', 'long', 'full'];
// The port's timestamps are naive, so they are read as UTC and print no zone.
const UTC = { timeZone: 'UTC' };
// One-digit day, month and hour, so a padded field shows as two digits.
const SAMPLE = new Date(Date.UTC(2026, 0, 5, 9, 7, 3));
const LETTER = /[A-Za-z\\]/g;

function tables(tag) {
  const numbering = new Intl.DateTimeFormat(tag).resolvedOptions().numberingSystem;
  const digitFormat = new Intl.NumberFormat(tag, { numberingSystem: numbering, useGrouping: false });
  const digits = [...Array(10).keys()].map((n) => digitFormat.format(n));
  const latin = (text) => [...text].map((ch) => (digits.includes(ch) ? String(digits.indexOf(ch)) : ch)).join('');
  const part = (options, date, type) =>
    new Intl.DateTimeFormat(tag, { ...UTC, ...options }).formatToParts(date).find((p) => p.type === type)?.value ?? '';

  // Each name is read from a whole date, so it is the form ICU prints beside a day.
  const months = [...Array(12).keys()].map((m) => part({ dateStyle: 'long' }, new Date(Date.UTC(2026, m, 5)), 'month'));
  const monthsShort = [...Array(12).keys()].map((m) => part({ dateStyle: 'medium' }, new Date(Date.UTC(2026, m, 5)), 'month'));
  // 2026-01-04 is a Sunday, the formatter's weekday zero.
  const weekdays = [...Array(7).keys()].map((d) => part({ dateStyle: 'full' }, new Date(Date.UTC(2026, 0, 4 + d)), 'weekday'));
  const dayPeriods = [9, 21].map((h) => part({ hour: 'numeric', hour12: true }, new Date(Date.UTC(2026, 0, 5, h)), 'dayPeriod'));

  const field = (type, value, hasDayPeriod) => {
    const v = latin(value);
    switch (type) {
      case 'day':
        return v === '5' ? 'j' : 'd';
      case 'month':
        if (v === '1') return 'n';
        if (v === '01') return 'm';
        if (value === months[0]) return 'F';
        if (value === monthsShort[0]) return 'M';
        break;
      case 'year':
        return v.length === 2 ? 'y' : 'Y';
      case 'weekday':
        if (value === weekdays[1]) return 'l';
        break;
      case 'hour':
        if (hasDayPeriod) return v === '9' ? 'g' : 'h';
        return v === '9' ? 'G' : 'H';
      case 'minute':
        return 'i';
      case 'second':
        return 's';
      case 'dayPeriod':
        return 'a';
    }
    throw new Error(`${tag}: no PHP character for ${type} "${value}"`);
  };

  const pattern = (options) => {
    let parts = new Intl.DateTimeFormat(tag, { ...UTC, ...options }).formatToParts(SAMPLE);
    // The zone and the literal that joins it go: a naive timestamp has no zone.
    const zone = parts.findIndex((p) => p.type === 'timeZoneName');
    if (zone >= 0) parts = parts.filter((_, i) => i !== zone && i !== (zone === 0 ? 1 : zone - 1));
    const hasDayPeriod = parts.some((p) => p.type === 'dayPeriod');
    return parts
      .map((p) => (p.type === 'literal' ? p.value.replace(LETTER, (ch) => `\\${ch}`) : field(p.type, p.value, hasDayPeriod)))
      .join('');
  };

  return {
    tag,
    numbering,
    months,
    monthsShort,
    weekdays,
    date: STYLES.map((style) => pattern({ dateStyle: style })),
    time: STYLES.map((style) => pattern({ timeStyle: style })),
    dayPeriods,
    digits,
  };
}

const rust = (text) => JSON.stringify(text);
const list = (items) => `[${items.map(rust).join(', ')}]`;
const chars = (items) => `[${items.map((d) => `'${d}'`).join(', ')}]`;

const statics = LOCALES.map(tables).map(
  (t) => `/// \`${t.tag}\`: ICU's names and \`dateStyle\`/\`timeStyle\` patterns, \`${t.numbering}\` digits.
pub(super) static ${t.tag.toUpperCase()}_NAMES: DateNames = DateNames {
    months: ${list(t.months)},
    months_short: ${list(t.monthsShort)},
    weekdays: ${list(t.weekdays)},
    short: ${rust(t.date[0])},
    medium: ${rust(t.date[1])},
    long: ${rust(t.date[2])},
    full: ${rust(t.date[3])},
    time_short: ${rust(t.time[0])},
    time_medium: ${rust(t.time[1])},
    time_long: ${rust(t.time[2])},
    time_full: ${rust(t.time[3])},
    day_periods: ${list(t.dayPeriods)},
    digits: ${chars(t.digits)},
};
`,
);

const { icu, cldr, node } = process.versions;
writeFileSync(
  OUT,
  `//! Generated by \`web/admin/scripts/cldr-dates.mjs\` from ICU ${icu} (CLDR ${cldr}, node ${node}).
//! Do not edit: run \`node scripts/cldr-dates.mjs\` in \`web/admin\` to rewrite it.
//!
//! Each table is what PHP \`intl\` would print for the locale: the CLDR names in
//! their format context and the CLDR patterns as PHP date characters. A
//! \`timeStyle\` pattern drops its zone, which a naive timestamp cannot supply.

use super::dates::DateNames;

${statics.join('\n')}`,
);
execFileSync('rustfmt', ['--edition', '2024', OUT], { stdio: 'inherit' });
console.log(`wrote ${OUT} from ICU ${icu} (CLDR ${cldr})`);
