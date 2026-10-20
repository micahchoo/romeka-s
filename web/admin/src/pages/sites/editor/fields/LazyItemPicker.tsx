/**
 * The item picker as its own chunk: the editor and the site's Items page load
 * it the first time they show it, so neither pays for it up front.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { useTranslator } from '../../../../i18n';
import { sites } from '../../../../i18n/namespaces/sites';
import type { ItemPickerProps } from './itemPickerModel';

const ItemPicker = lazy(() =>
  import('./ItemPicker').then((module) => ({ default: module.ItemPicker })),
);

export type { ItemPickerProps } from './itemPickerModel';

const Loading = (): ReactElement => {
  const { t } = useTranslator(sites);
  return (
    <p className="field-note" role="status">
      {t('itemPicker.loading')}
    </p>
  );
};

export const LazyItemPicker = (props: ItemPickerProps): ReactElement => (
  <Suspense fallback={<Loading />}>
    <ItemPicker {...props} />
  </Suspense>
);
