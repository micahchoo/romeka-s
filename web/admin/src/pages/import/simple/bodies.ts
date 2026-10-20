/**
 * The Simple screens' bodies, each a `lazy(() => import('./<screen>'))` that
 * reads `useDraft()`. A screen missing here shows its frame alone.
 */
import { lazy } from 'react';
import type { SimpleBodies } from '../bodies';

export const SIMPLE_BODIES: SimpleBodies = {
  files: lazy(() => import('./files')),
  found: lazy(() => import('./found')),
  example: lazy(() => import('./example')),
  'all-files': lazy(() => import('../lists/all-files')),
  list: lazy(() => import('./list')),
  pair: lazy(() => import('../lists/pair')),
  where: lazy(() => import('./where')),
  check: lazy(() => import('./check')),
  'all-items': lazy(() => import('../lists/all-items')),
  add: lazy(() => import('./add')),
};
