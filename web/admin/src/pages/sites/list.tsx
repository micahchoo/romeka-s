import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { formatDateTime } from '../../app/format';
import { ListPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 25;

/** `SiteAdmin/IndexController::indexAction`: the site browse table. */
export const SiteList = (): ReactElement => {
  const { t, locale } = useTranslator();
  const resource = t('resource.sites');
  return (
    <div>
      <p>
        <a href="#/sites/new">{t('sites.add')}</a>
      </p>
      <ListPage
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={(page) => api.listSites({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' })}
        rowKey={(site) => String(site.id)}
        columns={[
          {
            header: t('column.title'),
            cell: (site) => <a href={`#/sites/${String(site.id)}`}>{site.title}</a>,
          },
          { header: t('column.slug'), cell: (site) => site.slug },
          { header: t('column.theme'), cell: (site) => site.theme },
          { header: t('column.public'), cell: (site) => (site.isPublic ? t('common.yes') : t('common.no')) },
          { header: t('column.created'), cell: (site) => formatDateTime(site.created, locale) },
        ]}
      />
    </div>
  );
};
