/** The words the upload view shows for sizes, row states and problems. */
import type { Locale } from '../../../i18n';
import type { ImportTranslator } from '../messages';
import type { UploadProblem } from './problem';
import type { UploadRow } from './session';

const UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;
const formats = new Map<string, Intl.NumberFormat>();

/**
 * A size in the locale's unit words. The steps are 1000, as the unit words
 * mean them: 1 GB is 1,000,000,000 bytes, the number a limit setting holds.
 */
export const sizeText = (bytes: number, locale: Locale): string => {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const name = UNITS[unit] ?? 'byte';
  const key = `${locale}:${name}`;
  let format = formats.get(key);
  if (format === undefined) {
    format = new Intl.NumberFormat(locale, { style: 'unit', unit: name, unitDisplay: unit === 0 ? 'long' : 'short', maximumFractionDigits: unit < 2 ? 0 : 1 });
    formats.set(key, format);
  }
  return format.format(value);
};

export const problemText = (problem: UploadProblem, translator: ImportTranslator): string => {
  const { t, locale } = translator;
  switch (problem.kind) {
    case 'fileTooLarge':
      return t('upload.problem.fileTooLarge', { limit: sizeText(problem.limit, locale) });
    case 'importFull':
      return t('upload.problem.importFull', { limit: sizeText(problem.limit, locale), used: sizeText(problem.used, locale) });
    case 'badName':
      return t('upload.problem.badName');
    case 'importStarted':
      return t('upload.problem.importStarted');
    case 'notAllowed':
      return t('upload.problem.notAllowed');
    case 'gone':
      return t('upload.problem.gone');
    case 'zipRefused':
      return t('upload.problem.zipRefused', { reason: /[.!?]$/.test(problem.reason) ? problem.reason : `${problem.reason}.` });
    case 'connection':
      return t('upload.problem.connection');
    case 'server':
      return t('upload.problem.server', { message: problem.message });
  }
};

export const stateText = (row: UploadRow, translator: ImportTranslator): string => {
  const { t, locale } = translator;
  switch (row.phase.kind) {
    case 'waiting':
      return t('upload.state.waiting');
    case 'sending':
      return t('upload.state.sending');
    case 'paused':
      return t('upload.state.paused');
    case 'needsFile':
      return t('upload.state.needsFile', { sent: sizeText(row.sent, locale), size: sizeText(row.size, locale) });
    case 'unpacking':
      return t('upload.state.unpacking');
    case 'done':
      return t(row.phase.unpacked ? 'upload.state.unpacked' : 'upload.state.done');
    case 'failed':
      return problemText(row.phase.problem, translator);
  }
};

