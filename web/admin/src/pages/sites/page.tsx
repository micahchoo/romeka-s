import type { ReactElement } from 'react';
import { pageIdFromParam, siteIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { SiteCreatePage } from './create';
import { SiteDetail } from './detail';
import { SiteList } from './list';
import { SiteNavigationPage } from './navigation';
import { SitePageEditor } from './pageEditor';
import { SiteResourcesPage } from './resources';
import { SiteThemePage } from './theme';
import { SiteUsersPage } from './users';

/**
 * `#/sites` lists, `#/sites/new` creates, `#/sites/<id>` edits a site,
 * `#/sites/<id>/navigation`, `/resources`, `/users` and `/theme` edit one
 * member group each, `#/sites/<id>/pages/new` adds a page and
 * `#/sites/<id>/pages/<pageId>` edits one.
 */
export const SitesPage = ({
  param = null,
  rest = [],
}: {
  readonly param?: string | null;
  readonly rest?: readonly string[];
} = {}): ReactElement => {
  const { t } = useTranslator();
  if (param === null) {
    return <SiteList />;
  }
  if (param === 'new') {
    return <SiteCreatePage />;
  }
  const siteId = siteIdFromParam(param);
  if (siteId === null) {
    return <p role="alert">{t('route.invalidSiteId', { param })}</p>;
  }
  if (rest.length === 1) {
    return <SiteDetail key={String(siteId)} siteId={siteId} />;
  }
  if (rest.length === 2) {
    switch (rest[1]) {
      case 'navigation':
        return <SiteNavigationPage key={String(siteId)} siteId={siteId} />;
      case 'resources':
        return <SiteResourcesPage key={String(siteId)} siteId={siteId} />;
      case 'users':
        return <SiteUsersPage key={String(siteId)} siteId={siteId} />;
      case 'theme':
        return <SiteThemePage key={String(siteId)} siteId={siteId} />;
      default:
        return <p role="alert">{t('route.noSiteRoute', { param })}</p>;
    }
  }
  if (rest.length === 3 && rest[1] === 'pages') {
    const target = rest[2];
    if (target === undefined || target === 'new') {
      return <SitePageEditor key={`${String(siteId)}-new`} siteId={siteId} pageId={null} />;
    }
    const pageId = pageIdFromParam(target);
    if (pageId === null) {
      return <p role="alert">{t('route.invalidPageId', { param: target })}</p>;
    }
    return <SitePageEditor key={String(pageId)} siteId={siteId} pageId={pageId} />;
  }
  return <p role="alert">{t('route.noSiteRoute', { param })}</p>;
};
