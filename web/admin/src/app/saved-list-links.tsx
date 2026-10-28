/**
 * The signed-in user's saved lists under Items in the nav, each a link to the Items screen with
 * its query. The one open now is marked. The names come with the session (`GET /api/me`), so
 * this costs no request; the Items screen saves, renames and deletes them.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../i18n';
import { useSession } from './auth';
import { hashQuery, parseHash, useHash } from './router';

/**
 * The query of the Items screen open now, as a saved list stores it: without the page and the
 * view, which are where the reader is and how they look, not what the list is.
 */
const openQuery = (hash: string): string | null => {
  const route = parseHash(hash);
  if (route?.page !== 'items' || route.param !== null) {
    return null;
  }
  const query = hashQuery(hash);
  query.delete('page');
  query.delete('view');
  return query.toString();
};

export const SavedListLinks = (): ReactElement | null => {
  const { t } = useTranslator();
  const user = useSession();
  const open = openQuery(useHash(true));
  const lists = user?.savedLists ?? [];
  if (lists.length === 0) {
    return null;
  }
  return (
    <ul className="nav-list nav-saved" aria-label={t('nav.savedLists')}>
      {lists.map((list) => (
        <li key={list.name}>
          <a
            className="nav-link nav-saved-link"
            href={`#/items?${list.query}`}
            aria-current={list.query === open ? 'true' : undefined}
          >
            {list.name}
          </a>
        </li>
      ))}
    </ul>
  );
};
