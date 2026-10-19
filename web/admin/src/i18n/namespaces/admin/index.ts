/**
 * The admin page messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `AdminMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'jobs.status.starting': 'Starting',
  'jobs.status.stopping': 'Stopping',
  'jobs.status.in_progress': 'In Progress',
  'jobs.status.completed': 'Completed',
  'jobs.status.stopped': 'Stopped',
  'jobs.status.error': 'Error',
} as const;

/** A complete table of this namespace, in one locale. */
export type AdminMessages = Table<keyof typeof en>;

export const admin = defineNamespace('admin', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
