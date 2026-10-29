/**
 * The sites a user holds a site role on, each a link to that site's Users tab,
 * where the role is changed.
 *
 * A site role lives on the site (`o:site_permission`), so the list reads every
 * site the caller may see and keeps the ones that name this user. A site the
 * caller may not see is not read, so it is missing here like any other.
 */
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { Site, UserId } from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf } from '../../i18n';
import { admin } from '../../i18n/namespaces/admin';

const SITE_ROLE_LABELS: Readonly<Record<string, KeyOf<typeof admin>>> = {
  admin: 'users.siteRoleAdmin',
  editor: 'users.siteRoleEditor',
  viewer: 'users.siteRoleViewer',
};

interface UserSite {
  readonly site: Site;
  readonly role: string;
}

const sitesOf = async (userId: UserId): Promise<readonly UserSite[]> => {
  const sites = await allPages((page) =>
    api.listSites({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'title', sortOrder: 'asc' }),
  );
  return sites.flatMap((site) =>
    site.sitePermissions
      .filter((permission) => permission.user.id === userId)
      .map((permission) => ({ site, role: permission.role })),
  );
};

export const UserSites = ({ userId }: { readonly userId: UserId }): ReactElement => {
  const { t } = useTranslator(admin);
  const state = useAsync(() => sitesOf(userId), [userId]);
  return (
    <section aria-labelledby="user-sites-heading">
      <h2 id="user-sites-heading">{t('users.sitesHeading')}</h2>
      {state.status === 'loading' ? <p role="status">{t('users.sitesLoading')}</p> : null}
      {state.status === 'error' ? <ErrorNotice error={state.error} /> : null}
      {state.status === 'ready' && state.data.length === 0 ? <p>{t('users.noSites')}</p> : null}
      {state.status === 'ready' && state.data.length > 0 ? (
        <table>
          <thead>
            <tr>
              <th scope="col">{t('users.siteColumn')}</th>
              <th scope="col">{t('users.role')}</th>
            </tr>
          </thead>
          <tbody>
            {state.data.map(({ site, role }) => {
              const label = SITE_ROLE_LABELS[role];
              return (
                <tr key={String(site.id)}>
                  <td>
                    <a href={`#/sites/${String(site.id)}/users`}>{site.title}</a>
                  </td>
                  <td>{label === undefined ? role : t(label)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      ) : null}
    </section>
  );
};
