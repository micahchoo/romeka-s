import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { SiteCreateForm } from './editor';

/** `SiteAdmin/IndexController::addAction`: the new site form. */
export const SiteCreatePage = (): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => api.listUsers({ perPage: 200, sortBy: 'name' }), []);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.siteForm')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteCreateForm users={state.data.items} />;
};
