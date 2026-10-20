/**
 * The Advanced screens' bodies, each a `lazy(() => import('./<screen>'))` that
 * reads `useDraft()`. A screen missing here shows its frame alone.
 */
import { lazy } from 'react';
import type { AdvancedBodies } from '../bodies';
import { RUN_BODY } from '../run/bodies';

export const ADVANCED_BODIES: AdvancedBodies = {
  files: lazy(() => import('./files')),
  lists: lazy(() => import('./lists')),
  fields: lazy(() => import('./fields')),
  problems: lazy(() => import('./problems')),
  existing: lazy(() => import('./existing')),
  changes: lazy(() => import('./changes')),
  'all-files': lazy(() => import('../lists/all-files')),
  pair: lazy(() => import('../lists/pair')),
  items: lazy(() => import('../lists/all-items')),
  run: RUN_BODY,
};
