import type { ReactElement } from 'react';
import { api, defaultBaseUrl } from '../../api/client';
import { ErrorNotice } from '../../app/errors';
import { TableScroll } from '../../app/table-scroll';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/**
 * `Admin/SystemInfoController::browseAction`. The upstream page reports the
 * PHP and MySQL environment, which the port has no equivalent of. It reports
 * what the port serves: the Omeka S version from the response header and the
 * registered API resource names.
 */
export const SystemInfoPage = (): ReactElement => {
  const { t } = useTranslator(common, admin);
  const state = useAsync(() => api.getApiInfo(), []);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.systemInfo')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const info = state.data;
  return (
    <TableScroll label={t('systemInfo.tableLabel')}>
      <table>
        <tbody>
          <tr>
            <th scope="rowgroup" colSpan={2}>
              {t('systemInfo.omeka')}
            </th>
          </tr>
          <tr>
            <th scope="row">{t('systemInfo.version')}</th>
            <td>{info.version ?? t('common.unknown')}</td>
          </tr>
          <tr>
            <th scope="rowgroup" colSpan={2}>
              {t('systemInfo.api')}
            </th>
          </tr>
          <tr>
            <th scope="row">{t('systemInfo.baseUrl')}</th>
            <td>{defaultBaseUrl() || t('common.sameOrigin')}</td>
          </tr>
          <tr>
            <th scope="row">{t('systemInfo.resources')}</th>
            <td>{info.resources.map((resource) => resource.id).join(', ')}</td>
          </tr>
        </tbody>
      </table>
    </TableScroll>
  );
};
