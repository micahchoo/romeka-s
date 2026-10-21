/**
 * The field controls that load as chunks of their own, so the shared
 * `fields` chunk stays under its budget (`web/admin/PERF.md`): the content
 * blocks' item and item-media controls, the IIIF manifest source, which only
 * the IIIF presentation block uses, and the asset picker with its upload
 * control. Each chunk loads the first time a form shows one of its kinds; the
 * form says it is loading meanwhile.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { ControlLoading } from './ControlLoading';
import type { AssetPickerProps } from './AssetPicker';
import type { ContentControlProps } from './contentControls';
import type { FocalPointControlProps } from './focalPointControl';
import type { ManifestSourceFieldProps } from './ManifestSourceField';

const ContentControl = lazy(() =>
  import('./contentControls').then((module) => ({ default: module.ContentControl })),
);

const ManifestSource = lazy(() =>
  import('./ManifestSourceField').then((module) => ({ default: module.ManifestSourceField })),
);

const AssetPicker = lazy(() =>
  import('./AssetPicker').then((module) => ({ default: module.AssetPicker })),
);

const FocalPoint = lazy(() =>
  import('./focalPointControl').then((module) => ({ default: module.FocalPointControl })),
);

export const LazyContentControl = (props: ContentControlProps): ReactElement => (
  <Suspense fallback={<ControlLoading />}>
    <ContentControl {...props} />
  </Suspense>
);

export const LazyManifestSourceField = (props: ManifestSourceFieldProps): ReactElement => (
  <Suspense fallback={<ControlLoading />}>
    <ManifestSource {...props} />
  </Suspense>
);

export const LazyAssetPicker = (props: AssetPickerProps): ReactElement => (
  <Suspense fallback={<ControlLoading />}>
    <AssetPicker {...props} />
  </Suspense>
);

export const LazyFocalPointControl = (props: FocalPointControlProps): ReactElement => (
  <Suspense fallback={<ControlLoading />}>
    <FocalPoint {...props} />
  </Suspense>
);
