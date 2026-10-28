/**
 * The signed-in user's saved lists under Items in the nav, each a link to the Items screen with
 * its query and its name. The one open now is marked, also after its filters change. The names
 * come with the session (`GET /api/me`), so this costs no request; the Items screen saves,
 * updates, renames and deletes them.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../i18n';
import { useSession } from './auth';
import { LIST_KEY, openSavedList, savedListHref } from './open-saved-list';
import { hashQuery, parseHash, useHash } from './router';

/**
 * The name and query of the Items screen open now, the query as a saved list stores it: without
 * the page and the view, which are where the reader is and how they look, not what the list is.
 */
const openAddress = (hash: string): { readonly name: string | null; readonly query: string } | null => {
  const route = parseHash(hash);
  if (route?.page !== 'items' || route.param !== null) {
    return null;
  }
  const query = hashQuery(hash);
  const name = query.get(LIST_KEY);
  query.delete(LIST_KEY);
  query.delete('page');
  query.delete('view');
  return { name, query: query.toString() };
};

export const SavedListLinks = (): ReactElement | null => {
  const { t } = useTranslator();
  const user = useSession();
  const address = openAddress(useHash(true));
  const lists = user?.savedLists ?? [];
  if (lists.length === 0) {
    return null;
  }
  const open = address === null ? null : openSavedList(lists, address.name, address.query);
  return (
    <ul className="nav-list nav-saved" aria-label={t('nav.savedLists')}>
      {lists.map((list) => (
        <li key={list.name}>
          <a
            className="nav-link nav-saved-link"
            href={savedListHref(list)}
            aria-current={list.name === open?.list.name ? 'true' : undefined}
          >
            {list.name}
          </a>
        </li>
      ))}
    </ul>
  );
};
