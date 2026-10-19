import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { formatDateTime } from '../../app/format';
import { ListPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 25;

/** `Admin/UserController::browseAction`: email, name, role, active, created. */
export const UserList = (): ReactElement => {
  const { t, locale } = useTranslator();
  const resource = t('resource.users');
  return (
    <div>
      <p>
        <a href="#/users/new">{t('users.add')}</a>
      </p>
      <ListPage
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={(page) => api.listUsers({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' })}
        rowKey={(user) => String(user.id)}
        columns={[
          {
            header: t('column.name'),
            cell: (user) => <a href={`#/users/${String(user.id)}`}>{user.name}</a>,
          },
          { header: t('column.email'), cell: (user) => user.email },
          { header: t('column.role'), cell: (user) => user.role },
          { header: t('column.active'), cell: (user) => (user.isActive ? t('common.yes') : t('common.no')) },
          { header: t('column.created'), cell: (user) => formatDateTime(user.created, locale) },
        ]}
      />
    </div>
  );
};
