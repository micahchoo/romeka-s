/**
 * The account page messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `AccountMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'password.heading': 'Set a password',
  'password.new': 'New password',
  'password.confirm': 'Confirm the new password',
  'password.submit': 'Set the password',
  'password.errorEmpty': 'Enter a new password.',
  'password.errorMismatch': 'The two passwords do not match.',
  'password.done': 'Your password is set. Sign in from the header to continue.',
  'password.missingKey': 'This link carries no password key. Request a new one.',
  'forgot.heading': 'Reset your password',
  'forgot.submit': 'Send the reset link',
  'forgot.errorEmpty': 'Enter your email address.',
  'forgot.sent': 'Check your mail for the reset link.',
  'forgot.hint':
    'The answer is the same whether or not the address matches an account, so this page never says which addresses exist.',
} as const;

/** A complete table of this namespace, in one locale. */
export type AccountMessages = Table<keyof typeof en>;

export const account = defineNamespace('account', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
