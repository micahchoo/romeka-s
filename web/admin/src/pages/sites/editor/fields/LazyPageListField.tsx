/**
 * The page list control as its own chunk: only the list-of-pages block shows
 * it, so the editor's field controls do not pay for it until that block opens
 * (PERF.md, Decision 25).
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { ControlLoading } from './ControlLoading';
import type { PageListFieldProps } from './PageListField';

const PageListField = lazy(() =>
  import('./PageListField').then((module) => ({ default: module.PageListField })),
);

export const LazyPageListField = (props: PageListFieldProps): ReactElement => (
  <Suspense fallback={<ControlLoading />}>
    <PageListField {...props} />
  </Suspense>
);
