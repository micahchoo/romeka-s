/** The line a field control's Suspense boundary shows while its chunk loads. */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';

export const ControlLoading = (): ReactElement => {
  const { t } = useTranslator(common);
  return (
    <p className="field-note" role="status">
      {t('loading.form')}
    </p>
  );
};
