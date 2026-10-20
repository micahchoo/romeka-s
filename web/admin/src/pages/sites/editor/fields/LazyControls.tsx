/**
 * The field controls that load as chunks of their own, so the shared
 * `fields` chunk stays under its budget (`web/admin/PERF.md`): the content
 * blocks' item and item-media controls, and the IIIF manifest source, which
 * only the IIIF presentation block uses. Each chunk loads the first time a
 * form shows one of its kinds; the form says it is loading meanwhile.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import type { ContentControlProps } from './contentControls';
import type { ManifestSourceFieldProps } from './ManifestSourceField';

const ContentControl = lazy(() =>
  import('./contentControls').then((module) => ({ default: module.ContentControl })),
);

const ManifestSource = lazy(() =>
  import('./ManifestSourceField').then((module) => ({ default: module.ManifestSourceField })),
);

const Loading = (): ReactElement => {
  const { t } = useTranslator(common);
  return (
    <p className="field-note" role="status">
      {t('loading.form')}
    </p>
  );
};

export const LazyContentControl = (props: ContentControlProps): ReactElement => (
  <Suspense fallback={<Loading />}>
    <ContentControl {...props} />
  </Suspense>
);

export const LazyManifestSourceField = (props: ManifestSourceFieldProps): ReactElement => (
  <Suspense fallback={<Loading />}>
    <ManifestSource {...props} />
  </Suspense>
);
