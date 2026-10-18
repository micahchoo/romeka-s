/**
 * The admin page table, read from the source the app builds it from.
 *
 * The e2e tsconfig includes only `e2e/`, so a spec cannot import `src/`. The
 * page-load spec and the accessibility spec both read the `AdminPageId` union
 * through here, so neither carries its own copy of the parse.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { APP_ROOT } from './state';

const TABLE_SOURCE = join(APP_ROOT, 'src', 'app', 'pages.tsx');

/** The members of the `AdminPageId` union in `src/app/pages.tsx`. */
export const adminPageIds = (): readonly string[] => {
  const union = /export type AdminPageId =([\s\S]*?);/.exec(readFileSync(TABLE_SOURCE, 'utf8'));
  const body = union?.[1];
  if (body === undefined) {
    throw new Error(`no AdminPageId union in ${TABLE_SOURCE}`);
  }
  return [...body.matchAll(/'([^']+)'/g)].map((match) => match[1] ?? '');
};
