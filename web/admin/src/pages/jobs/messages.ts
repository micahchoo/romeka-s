/**
 * The Jobs page's own messages: the status labels. They load with the page
 * chunk rather than with the main catalogue, which every first paint pays for
 * (`perf/budget.json`). Six short strings a locale are too little to fetch on
 * their own, so every locale is imported here. Each table is typed against
 * `en`, so a key added in English and missing elsewhere is a `tsc` error.
 */
import { useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale } from '../../i18n';
import { hi } from './locales/hi';
import { kn } from './locales/kn';
import { ml } from './locales/ml';
import { mr } from './locales/mr';
import { ne } from './locales/ne';
import { ta } from './locales/ta';
import { te } from './locales/te';

/** `JobRepresentation::$statusLabels`, keyed by the stored `Job::STATUS_*`. */
export const en = {
  'jobs.status.starting': 'Starting',
  'jobs.status.stopping': 'Stopping',
  'jobs.status.in_progress': 'In Progress',
  'jobs.status.completed': 'Completed',
  'jobs.status.stopped': 'Stopped',
  'jobs.status.error': 'Error',
} as const;

type JobsMessageKey = keyof typeof en;

/** A complete Jobs page table. */
export type JobsMessages = Readonly<Record<JobsMessageKey, string>>;

// `application/language/es.po`.
const es: JobsMessages = {
  'jobs.status.starting': 'Iniciando',
  'jobs.status.stopping': 'Deteniendo',
  'jobs.status.in_progress': 'En proceso',
  'jobs.status.completed': 'Completado',
  'jobs.status.stopped': 'Parado',
  'jobs.status.error': 'Error',
};

const MESSAGES: Readonly<Record<Locale, JobsMessages>> = { en, es, hi, mr, ml, ta, kn, te, ne };

const isStatusKey = (key: string): key is JobsMessageKey => Object.hasOwn(en, key);

/**
 * `JobRepresentation::statusLabel` through `@translate`: the label of a stored
 * status in the current locale, or `null` for a status upstream has no label
 * for, which the caller prints as unknown.
 */
export const useJobStatusLabel = (): ((status: string | null) => string | null) => {
  const locale = useLocale();
  return useMemo(() => {
    const messages = MESSAGES[locale];
    return (status: string | null): string | null => {
      const key = `jobs.status.${status ?? ''}`;
      return isStatusKey(key) ? messages[key] : null;
    };
  }, [locale]);
};
