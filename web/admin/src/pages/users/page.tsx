import type { ReactElement } from 'react';
import { userIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { UserForm } from './form';
import { UserList } from './list';
import { admin } from '../../i18n/namespaces/admin';

/** `#/users` lists, `#/users/new` creates, `#/users/<id>` edits. */
export const UsersPage = ({ param = null }: { readonly param?: string | null } = {}): ReactElement => {
  const { t } = useTranslator(admin);
  if (param === null) {
    return <UserList />;
  }
  if (param === 'new') {
    return <UserForm userId={null} />;
  }
  const userId = userIdFromParam(param);
  if (userId === null) {
    return <p role="alert">{t('route.invalidUserId', { param })}</p>;
  }
  return <UserForm key={String(userId)} userId={userId} />;
};
