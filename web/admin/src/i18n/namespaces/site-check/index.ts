/**
 * The site check's messages: the panel on the site hub that counts, before a
 * curator shares the site's link, what visitors meet on its public pages.
 *
 * English is here, in the hub's chunk; every other locale is its own chunk
 * under `locales/`. Each locale file is typed `SiteCheckMessages`, so a key
 * added here and missing there is a `tsc` error. A line has three forms
 * because the catalogue has no plural rules: one block is always on one page.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'siteCheck.title': 'Before you share this site',
  'siteCheck.intro': 'Counts what visitors meet on this site\'s public pages.',
  'siteCheck.run': 'Check this site',
  'siteCheck.again': 'Check again',
  'siteCheck.running': 'Checking the site…',
  'siteCheck.nothing': 'Nothing to check.',
  'siteCheck.unwrittenOne': '1 block not written, on 1 public page',
  'siteCheck.unwrittenOnePage': '{count} blocks not written, on 1 public page',
  'siteCheck.unwritten': '{count} blocks not written, on {pages} public pages',
  'siteCheck.emptyListsOne': '1 list shows nothing to visitors, on 1 public page',
  'siteCheck.emptyListsOnePage': '{count} lists show nothing to visitors, on 1 public page',
  'siteCheck.emptyLists': '{count} lists show nothing to visitors, on {pages} public pages',
  'siteCheck.privateOne': '1 block points to private items or media, on 1 public page',
  'siteCheck.privateOnePage': '{count} blocks point to private items or media, on 1 public page',
  'siteCheck.private': '{count} blocks point to private items or media, on {pages} public pages',
  'siteCheck.privateNote': 'Visitors do not see private items or media.',
  'siteCheck.outsideMenuOne': '1 public page is not in the menu',
  'siteCheck.outsideMenu': '{count} public pages are not in the menu',
  'siteCheck.outsideMenuNote': 'Visitors reach a page outside the menu only by its link.',
} as const;

/** A complete table of this namespace, in one locale. */
export type SiteCheckMessages = Table<keyof typeof en>;

export const siteCheck = defineNamespace('site-check', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
