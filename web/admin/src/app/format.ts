import { ApiError } from '../api/client';
import type { Translator } from '../i18n';

/** The upstream message with its status suffix, through the catalogue. */
export const formatError = (error: Error, translator: Translator): string =>
  error instanceof ApiError
    ? `${error.message}${translator.t('format.httpSuffix', { status: error.status })}`
    : error.message;

/** ISO 8601 to `YYYY-MM-DD HH:MM` in UTC, so tests and servers agree. */
export const formatDateTime = (iso: string): string => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return date.toISOString().slice(0, 16).replace('T', ' ');
};

/** `MediaController::showAction` prints a media size as `%s bytes`. */
export const formatBytes = (size: number, translator: Translator): string =>
  translator.t('format.bytes', { size: translator.number(size) });
