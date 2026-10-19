/**
 * The frame every site screen sits in: the breadcrumb, a heading that names
 * what the screen shows, the link to the public site and the tab bar of the
 * site's screens (upstream's `site` navigation in `navigation.config.php`).
 *
 * The router keeps one frame mounted while the user moves between a site's
 * screens. The frame reads the site on its own, beside the screen's own reads,
 * so it never delays them: every authenticated request costs one bcrypt check
 * and parallel requests share it (`web/admin/PERF.md`).
 */
import { createContext, useContext, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { api, defaultBaseUrl } from '../../api/client';
import type { PageId, Site, SiteId } from '../../api/types';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { sites } from '../../i18n/namespaces/sites';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof sites>;

export type SiteScreen = 'pages' | 'navigation' | 'resources' | 'users' | 'theme' | 'settings';

/** What the frame holds: one of the site's screens, or one page in the editor. */
export type SiteView =
  | { readonly kind: 'screen'; readonly screen: SiteScreen }
  | { readonly kind: 'page'; readonly pageId: PageId | null };

export const SITE_TABS: readonly {
  readonly screen: SiteScreen;
  readonly segment: string;
  readonly labelKey: MessageId;
}[] = [
  { screen: 'pages', segment: '', labelKey: 'sites.pages' },
  { screen: 'navigation', segment: '/navigation', labelKey: 'page.navigation' },
  { screen: 'resources', segment: '/resources', labelKey: 'page.siteResources' },
  { screen: 'users', segment: '/users', labelKey: 'page.siteUsers' },
  { screen: 'theme', segment: '/theme', labelKey: 'page.siteTheme' },
  { screen: 'settings', segment: '/settings', labelKey: 'page.siteSettings' },
];

export const siteHref = (siteId: SiteId, screen: SiteScreen = 'pages'): string =>
  `#/sites/${String(siteId)}${SITE_TABS.find((tab) => tab.screen === screen)?.segment ?? ''}`;

/** The public address of a site, or of one of its pages. */
export const publicUrl = (siteSlug: string, pageSlug: string | null = null): string => {
  const site = `${defaultBaseUrl()}/s/${encodeURIComponent(siteSlug)}`;
  return pageSlug === null ? site : `${site}/page/${encodeURIComponent(pageSlug)}`;
};

export interface Crumb {
  readonly label: string;
  /** `null` for the last crumb, the screen the user is on. */
  readonly href: string | null;
}

export const Breadcrumb = ({ crumbs }: { readonly crumbs: readonly Crumb[] }): ReactElement => {
  const { t } = useTranslator(sites);
  return (
    <nav className="breadcrumb" aria-label={t('siteHub.breadcrumb')}>
      <ol>
        {crumbs.map((crumb, index) => (
          <li key={String(index)}>
            {crumb.href === null ? (
              <span aria-current="page">{crumb.label}</span>
            ) : (
              <a href={crumb.href}>{crumb.label}</a>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
};

/** Lets a screen that saved the site update the title the frame shows. */
const SiteSavedContext = createContext<(site: Site) => void>(() => undefined);

export const useSiteSaved = (): ((site: Site) => void) => useContext(SiteSavedContext);

const SiteTabs = ({
  siteId,
  view,
}: {
  readonly siteId: SiteId;
  readonly view: SiteView;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const current = (screen: SiteScreen): 'page' | 'true' | undefined => {
    if (view.kind === 'screen') {
      return view.screen === screen ? 'page' : undefined;
    }
    return screen === 'pages' ? 'true' : undefined;
  };
  return (
    <nav className="site-tabs" aria-label={t('sites.sectionsLabel')}>
      <ul>
        {SITE_TABS.map((tab) => (
          <li key={tab.screen}>
            <a href={siteHref(siteId, tab.screen)} aria-current={current(tab.screen)}>
              {t(tab.labelKey)}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
};

export const SiteFrame = ({
  siteId,
  view,
  children,
}: {
  readonly siteId: SiteId;
  readonly view: SiteView;
  readonly children: ReactNode;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const loaded = useAsync(() => api.getSite(siteId), [siteId]);
  const [saved, setSaved] = useState<Site | null>(null);
  const pageId = view.kind === 'page' ? view.pageId : null;
  const page = useAsync(
    async () => (pageId === null ? null : (await api.getSitePage(pageId)).title),
    [pageId],
  );
  const site = saved ?? (loaded.status === 'ready' ? loaded.data : null);
  const siteTitle = site?.title ?? t('siteHub.siteFallback', { id: siteId });

  const screenTitle = (): string | null => {
    if (view.kind === 'page') {
      if (view.pageId === null) {
        return t('page.add');
      }
      return page.status === 'ready' && page.data !== null
        ? page.data
        : t('common.pageFallback', { id: view.pageId });
    }
    const tab = SITE_TABS.find((entry) => entry.screen === view.screen);
    return view.screen === 'pages' || tab === undefined ? null : t(tab.labelKey);
  };
  const title = screenTitle();

  return (
    <div className="site-frame">
      <Breadcrumb
        crumbs={[
          { label: t('page.sites'), href: '#/sites' },
          { label: siteTitle, href: title === null ? null : siteHref(siteId) },
          ...(title === null ? [] : [{ label: title, href: null }]),
        ]}
      />
      <header className="site-frame-header">
        <h2>{title ?? siteTitle}</h2>
        {site === null ? null : (
          <a className="site-frame-view" href={publicUrl(site.slug)} target="_blank" rel="noreferrer">
            {t('siteHub.viewSite')}
          </a>
        )}
      </header>
      <SiteTabs siteId={siteId} view={view} />
      <SiteSavedContext.Provider value={setSaved}>{children}</SiteSavedContext.Provider>
    </div>
  );
};
