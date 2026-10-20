/**
 * The page list control as its own chunk: only the list-of-pages block shows
 * it, so the editor's field controls do not pay for it until that block opens
 * (PERF.md, Decision 25).
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import type { PageListFieldProps } from './PageListField';

const PageListField = lazy(() =>
  import('./PageListField').then((module) => ({ default: module.PageListField })),
);

const Loading = (): ReactElement => {
  const { t } = useTranslator(common);
  return (
    <p className="field-note" role="status">
      {t('loading.form')}
    </p>
  );
};

export const LazyPageListField = (props: PageListFieldProps): ReactElement => (
  <Suspense fallback={<Loading />}>
    <PageListField {...props} />
  </Suspense>
);
