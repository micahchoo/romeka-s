import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { pageIdFromParam, siteIdFromParam } from '../../api/parse';
import type { SiteId } from '../../api/types';
import { useTranslator } from '../../i18n';
import { SiteCreatePage } from './create';
import { SiteDetail } from './detail';
import { SiteList } from './list';
import { SiteSettingsPage } from './settings';
import { SiteFrame, SITE_TABS } from './SiteFrame';
import type { SiteView } from './SiteFrame';
import { SiteUsersPage } from './users';
import { sites } from '../../i18n/namespaces/sites';

/**
 * The page editor is its own chunk. It pulls in the canvas and the block forms
 * as two further chunks, so the sites route does not carry the editor.
 */
const SitePageEditor = lazy(() =>
  import('./editor/shell').then((module) => ({ default: module.SitePageEditor })),
);

/**
 * The Navigation and Site resources screens are their own chunks: they carry
 * the link tree and the item query builder, which the hub does not need.
 */
const SiteNavigationPage = lazy(() =>
  import('./navigation').then((module) => ({ default: module.SiteNavigationPage })),
);

const SiteResourcesPage = lazy(() =>
  import('./resources').then((module) => ({ default: module.SiteResourcesPage })),
);

/**
 * The Theme page is its own chunk: it carries the token controls, the contrast
 * readout and the preview panel, which the sites list does not need.
 */
const SiteThemePage = lazy(() =>
  import('./theme').then((module) => ({ default: module.SiteThemePage })),
);

type SiteRoute =
  | { readonly kind: 'view'; readonly view: SiteView }
  | { readonly kind: 'badPage'; readonly param: string }
  | { readonly kind: 'unknown' };

/**
 * `#/sites/<id>` is the hub, `#/sites/<id>/<segment>` one of the tabs,
 * `#/sites/<id>/pages/new` and `#/sites/<id>/pages/<pageId>` the page editor.
 */
const siteRoute = (rest: readonly string[]): SiteRoute => {
  if (rest.length === 1) {
    return { kind: 'view', view: { kind: 'screen', screen: 'pages' } };
  }
  if (rest.length === 2) {
    const tab = SITE_TABS.find((entry) => entry.segment === `/${rest[1] ?? ''}`);
    return tab === undefined
      ? { kind: 'unknown' }
      : { kind: 'view', view: { kind: 'screen', screen: tab.screen } };
  }
  if (rest.length === 3 && rest[1] === 'pages') {
    const target = rest[2] ?? 'new';
    if (target === 'new') {
      return { kind: 'view', view: { kind: 'page', pageId: null } };
    }
    const pageId = pageIdFromParam(target);
    return pageId === null
      ? { kind: 'badPage', param: target }
      : { kind: 'view', view: { kind: 'page', pageId } };
  }
  return { kind: 'unknown' };
};

const SiteScreenView = ({
  siteId,
  view,
}: {
  readonly siteId: SiteId;
  readonly view: SiteView;
}): ReactElement => {
  const { t } = useTranslator(sites);
  if (view.kind === 'page') {
    return (
      <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
        <SitePageEditor
          key={view.pageId === null ? `${String(siteId)}-new` : String(view.pageId)}
          siteId={siteId}
          pageId={view.pageId}
        />
      </Suspense>
    );
  }
  switch (view.screen) {
    case 'pages':
      return <SiteDetail key={String(siteId)} siteId={siteId} />;
    case 'navigation':
      return (
        <Suspense fallback={<p role="status">{t('loading.site')}</p>}>
          <SiteNavigationPage key={String(siteId)} siteId={siteId} />
        </Suspense>
      );
    case 'resources':
      return (
        <Suspense fallback={<p role="status">{t('loading.site')}</p>}>
          <SiteResourcesPage key={String(siteId)} siteId={siteId} />
        </Suspense>
      );
    case 'users':
      return <SiteUsersPage key={String(siteId)} siteId={siteId} />;
    case 'theme':
      return (
        <Suspense fallback={<p role="status">{t('loading.site')}</p>}>
          <SiteThemePage key={String(siteId)} siteId={siteId} />
        </Suspense>
      );
    case 'settings':
      return <SiteSettingsPage key={String(siteId)} siteId={siteId} />;
  }
};

/** `#/sites` lists, `#/sites/new` creates, `#/sites/<id>/...` is one site's frame. */
export const SitesPage = ({
  param = null,
  rest = [],
}: {
  readonly param?: string | null;
  readonly rest?: readonly string[];
} = {}): ReactElement => {
  const { t } = useTranslator(sites);
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
  const route = siteRoute(rest);
  if (route.kind === 'badPage') {
    return <p role="alert">{t('route.invalidPageId', { param: route.param })}</p>;
  }
  if (route.kind === 'unknown') {
    return <p role="alert">{t('route.noSiteRoute', { param })}</p>;
  }
  return (
    <SiteFrame key={String(siteId)} siteId={siteId} view={route.view}>
      <SiteScreenView siteId={siteId} view={route.view} />
    </SiteFrame>
  );
};
