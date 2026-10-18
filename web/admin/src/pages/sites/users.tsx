/**
 * The site users page (`SiteAdmin/IndexController::usersAction`).
 *
 * Upstream lists every site permission as a user and a role and saves the whole
 * list as `o:site_permission`; `SiteAdapter::hydrate` keeps the rows the body
 * names, changes their roles and removes the rest. An entry missing a user or a
 * role is skipped, so a half-filled row is dropped rather than rejected. The
 * three roles and their labels are upstream's (`site-admin/index/users.phtml`).
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { encodeSitePermissions } from '../../api/write';
import type { Site, SiteId, SiteRole, User, UserId } from '../../api/types';
import { SITE_ROLES } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { userOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';

const ROLE_LABELS: Readonly<Record<string, MessageKey>> = {
  viewer: 'siteRole.viewer',
  editor: 'siteRole.editor',
  admin: 'siteRole.admin',
};

interface PermissionDraft {
  readonly key: number;
  readonly userId: UserId | null;
  readonly role: string;
}

let nextKey = 0;

const draft = (userId: UserId | null, role: string): PermissionDraft => {
  nextKey += 1;
  return { key: nextKey, userId, role };
};

const roleLabel = (role: string): string => ROLE_LABELS[role] ?? role;

const isSiteRole = (value: string): value is SiteRole =>
  SITE_ROLES.some((role) => role === value);

interface Loaded {
  readonly site: Site;
  readonly users: readonly User[];
}

const load = async (siteId: SiteId): Promise<Loaded> => {
  const [site, users] = await Promise.all([
    api.getSite(siteId),
    api.listUsers({ perPage: 200, sortBy: 'name' }),
  ]);
  return { site, users: users.items };
};

const SiteUsersEditor = ({
  siteId,
  loaded,
}: {
  readonly siteId: SiteId;
  readonly loaded: Loaded;
}): ReactElement => {
  const { t } = useTranslator();
  const [rows, setRows] = useState<readonly PermissionDraft[]>(() =>
    loaded.site.sitePermissions.map((permission) => draft(permission.user.id, permission.role)),
  );
  const [pending, setPending] = useState('');
  const { state: saveState, save } = useSave();
  const options = userOptions(loaded.users);

  const patch = (key: number, change: Partial<PermissionDraft>): void => {
    setRows((current) =>
      current.map((row) => (row.key === key ? { ...row, ...change } : row)),
    );
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    const permissions = rows.flatMap((row) =>
      row.userId === null || row.role === ''
        ? []
        : [{ userId: row.userId, role: row.role }],
    );
    save(async () => {
      await api.patchSite(siteId, encodeSitePermissions(permissions));
    });
  };

  return (
    <form onSubmit={submit}>
      <table>
        <caption>{t('sites.usersLegend')}</caption>
        <thead>
          <tr>
            <th scope="col">{t('sites.user')}</th>
            <th scope="col">{t('sites.role')}</th>
            <th scope="col">{t('sites.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.key}>
              <td>
                <label htmlFor={`permission-user-${String(row.key)}`}>{t('sites.user')}</label>
                <select
                  id={`permission-user-${String(row.key)}`}
                  value={row.userId === null ? '' : String(row.userId)}
                  onChange={(event) => {
                    const selected = options.find(
                      (option) => String(option.id) === event.target.value,
                    );
                    patch(row.key, { userId: selected?.id ?? null });
                  }}
                >
                  <option value="">{t('sites.userSelect')}</option>
                  {row.userId === null ? null : (
                    <option value={String(row.userId)}>{String(row.userId)}</option>
                  )}
                  {options.map((option) => (
                    <option key={String(option.id)} value={String(option.id)}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <label htmlFor={`permission-role-${String(row.key)}`}>{t('sites.role')}</label>
                <select
                  id={`permission-role-${String(row.key)}`}
                  value={row.role}
                  onChange={(event) => {
                    patch(row.key, { role: event.target.value });
                  }}
                >
                  {isSiteRole(row.role) ? null : (
                    <option value={row.role}>{roleLabel(row.role)}</option>
                  )}
                  {SITE_ROLES.map((role) => (
                    <option key={role} value={role}>
                      {t(ROLE_LABELS[role] ?? 'siteRole.viewer')}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <button
                  type="button"
                  onClick={() => {
                    setRows((current) => current.filter((entry) => entry.key !== row.key));
                  }}
                >
                  {t('action.remove')}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p>
        <label htmlFor="permission-add-user">{t('sites.addUser')}</label>
        <select
          id="permission-add-user"
          value={pending}
          onChange={(event) => {
            setPending(event.target.value);
          }}
        >
          <option value="">{t('sites.userSelect')}</option>
          {options.map((option) => (
            <option key={String(option.id)} value={String(option.id)}>
              {option.label}
            </option>
          ))}
        </select>
        <button
          type="button"
          disabled={pending === ''}
          onClick={() => {
            const user = options.find((option) => String(option.id) === pending);
            if (user !== undefined) {
              setRows((current) => [...current, draft(user.id, 'viewer')]);
              setPending('');
            }
          }}
        >
          {t('sites.addUser')}
        </button>
      </p>
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

/** Loads the site permissions and the user list, then renders the editor. */
export const SiteUsersPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteUsersEditor key={String(siteId)} siteId={siteId} loaded={state.data} />;
};
