/**
 * The Simple screens' bodies, each a `lazy(() => import('./<screen>'))` that
 * reads `useDraft()`. A screen missing here shows its frame alone.
 */
import { lazy } from 'react';
import type { SimpleBodies } from '../bodies';

export const SIMPLE_BODIES: SimpleBodies = {
  files: lazy(() => import('./files')),
  structure: lazy(() => import('./structure')),
  'all-files': lazy(() => import('../lists/all-files')),
  pair: lazy(() => import('../lists/pair')),
  check: lazy(() => import('./check')),
  'all-items': lazy(() => import('../lists/all-items')),
  add: lazy(() => import('./add')),
};
