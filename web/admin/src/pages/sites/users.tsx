/**
 * The site users page (`SiteAdmin/IndexController::usersAction`).
 *
 * Upstream lists every site permission as a user and a role and saves the whole
 * list as `o:site_permission`; `SiteAdapter::hydrate` keeps the rows the body
 * names, changes their roles and removes the rest. Upstream skips an entry
 * missing a user; this tab flags such a row and holds the save instead, so a
 * row the curator sees is never dropped without a word. Each user appears in
 * at most one row. The three roles and their labels are upstream's
 * (`site-admin/index/users.phtml`).
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { encodeSitePermissions } from '../../api/write';
import type { Site, SiteId, SiteRole, User, UserId } from '../../api/types';
import { SITE_ROLES } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';
import { SaveBar, useUnsavedTab } from './unsaved';
import { AccessNote, useSiteAccess } from './SiteFrame';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof sites>;

const ROLE_LABELS: Readonly<Record<string, MessageId>> = {
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
  const { t } = useTranslator(common, sites);
  const [rows, setRows] = useState<readonly PermissionDraft[]>(() =>
    loaded.site.sitePermissions.map((permission) => draft(permission.user.id, permission.role)),
  );
  const [pending, setPending] = useState('');
  const [flagged, setFlagged] = useState(false);
  const { state: saveState, save } = useSave();
  const held = new Set(rows.flatMap((row) => (row.userId === null ? [] : [row.userId])));
  const unlisted = loaded.users.filter((user) => !held.has(user.id));
  const unsaved = useUnsavedTab(rows.map((row) => [row.userId, row.role]));

  const patch = (key: number, change: Partial<PermissionDraft>): void => {
    setRows((current) =>
      current.map((row) => (row.key === key ? { ...row, ...change } : row)),
    );
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    const permissions = rows.flatMap((row) =>
      row.userId === null ? [] : [{ userId: row.userId, role: row.role }],
    );
    if (permissions.length < rows.length) {
      setFlagged(true);
      return;
    }
    setFlagged(false);
    save(async () => {
      await api.patchSite(siteId, encodeSitePermissions(permissions));
      unsaved.saved();
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
                  aria-invalid={flagged && row.userId === null}
                  onChange={(event) => {
                    const selected = loaded.users.find(
                      (user) => String(user.id) === event.target.value,
                    );
                    patch(row.key, { userId: selected?.id ?? null });
                  }}
                >
                  <option value="">{t('sites.userSelect')}</option>
                  {row.userId === null || loaded.users.some((user) => user.id === row.userId) ? null : (
                    // A user the list does not hold (beyond its page, or not readable) keeps its row.
                    <option value={String(row.userId)}>{t('sites.userFallback', { id: row.userId })}</option>
                  )}
                  {loaded.users
                    .filter((user) => user.id === row.userId || !held.has(user.id))
                    .map((user) => (
                      <option key={String(user.id)} value={String(user.id)}>
                        {user.name}
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
          {unlisted.map((user) => (
            <option key={String(user.id)} value={String(user.id)}>
              {user.name}
            </option>
          ))}
        </select>
        <button
          type="button"
          disabled={pending === ''}
          onClick={() => {
            const user = unlisted.find((option) => String(option.id) === pending);
            if (user !== undefined) {
              setRows((current) => [...current, draft(user.id, 'viewer')]);
              setPending('');
            }
          }}
        >
          {t('sites.addUser')}
        </button>
      </p>
      {flagged && rows.some((row) => row.userId === null) ? (
        <p role="alert">{t('sites.errorRowNoUser')}</p>
      ) : null}
      <SaveStatus state={saveState} />
      <SaveBar dirty={unsaved.dirty}>
        <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
      </SaveBar>
    </form>
  );
};

/** Loads the site permissions and the user list, then renders the editor. */
export const SiteUsersPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(() => load(siteId), [siteId]);
  const access = useSiteAccess();
  if (access?.has('changeSite') === false) {
    return <AccessNote>{t('siteAccess.users')}</AccessNote>;
  }
  if (state.status === 'loading' || access === null) {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteUsersEditor key={String(siteId)} siteId={siteId} loaded={state.data} />;
};
