/**
 * The Simple screens' bodies, each a `lazy(() => import('./<screen>'))` that
 * reads `useDraft()`. A screen missing here shows its frame alone.
 */
import { lazy } from 'react';
import type { SimpleBodies } from '../bodies';
import { RUN_BODY } from '../run/bodies';

export const SIMPLE_BODIES: SimpleBodies = {
  'all-files': lazy(() => import('../lists/all-files')),
  pair: lazy(() => import('../lists/pair')),
  'all-items': lazy(() => import('../lists/all-items')),
  add: RUN_BODY,
};
