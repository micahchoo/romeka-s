/**
 * Which component renders each step. Each step is its own chunk, fetched the
 * first time it opens, so the Import page's chunk stays inside the per-chunk
 * budget as steps grow.
 */
import { lazy } from 'react';
import type { ComponentType } from 'react';
import type { TabProps, TabSlug } from './tabs';

export const STEPS: Readonly<Record<TabSlug, ComponentType<TabProps>>> = {
  folder: lazy(() => import('./folder').then((module) => ({ default: module.FolderStep }))),
  sets: lazy(() => import('./sets').then((module) => ({ default: module.SetsStep }))),
  types: lazy(() => import('./types').then((module) => ({ default: module.TypesStep }))),
  sources: lazy(() => import('./sources').then((module) => ({ default: module.SourcesStep }))),
  grid: lazy(() => import('./grid').then((module) => ({ default: module.GridStep }))),
  check: lazy(() => import('./check').then((module) => ({ default: module.CheckStep }))),
  run: lazy(() => import('./run').then((module) => ({ default: module.RunStep }))),
};
