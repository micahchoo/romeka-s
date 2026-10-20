/**
 * The frame every site screen sits in: the breadcrumb, a heading that names
 * what the screen shows, the link to the public site and the tab bar of the
 * site's screens (upstream's `site` navigation in `navigation.config.php`).
 *
 * The router keeps one frame mounted while the user moves between a site's
 * screens. The frame reads the site on its own, beside the screen's own reads,
 * so it never delays them: every authenticated request costs one bcrypt check
 * and parallel requests share it (`web/admin/PERF.md`).
 *
 * The frame is also where a screen learns what the signed-in user may do to
 * the site (`siteAccess.ts`): it reads the site's permissions once and hands
 * the capability set down, `null` until the site has arrived.
 */
import { createContext, useContext, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { api, defaultBaseUrl } from '../../api/client';
import type { PageId, Site, SiteId } from '../../api/types';
import { useSession } from '../../app/auth';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { sites } from '../../i18n/namespaces/sites';
import { FULL_ACCESS, siteAccess } from './siteAccess';
import type { SiteCapability } from './siteAccess';

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
  /** A tab whose whole purpose is this write is left out of the bar without it. */
  readonly needs?: SiteCapability;
}[] = [
  { screen: 'pages', segment: '', labelKey: 'sites.pages' },
  { screen: 'navigation', segment: '/navigation', labelKey: 'page.navigation' },
  { screen: 'resources', segment: '/resources', labelKey: 'page.siteResources' },
  { screen: 'users', segment: '/users', labelKey: 'page.siteUsers', needs: 'changeSite' },
  { screen: 'theme', segment: '/theme', labelKey: 'page.siteTheme', needs: 'changeSite' },
  { screen: 'settings', segment: '/settings', labelKey: 'page.siteSettings', needs: 'changeSite' },
];

export const siteHref = (siteId: SiteId, screen: SiteScreen = 'pages'): string =>
  `#/sites/${String(siteId)}${SITE_TABS.find((tab) => tab.screen === screen)?.segment ?? ''}`;

/** The name a site's theme goes by: the bundled theme's label, else its id. */
export const themeLabel = (theme: string, t: (key: 'sites.defaultThemeLabel') => string): string =>
  theme === 'default' ? t('sites.defaultThemeLabel') : theme;

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

/** Outside a frame every control is offered, and the server decides. */
const SiteAccessContext = createContext<ReadonlySet<SiteCapability> | null>(FULL_ACCESS);

/** What the signed-in user may do to the framed site; `null` while the frame reads it. */
export const useSiteAccess = (): ReadonlySet<SiteCapability> | null => useContext(SiteAccessContext);

/** The one line that says why a control is missing. */
export const AccessNote = ({ children }: { readonly children: string }): ReactElement => (
  <p className="site-access-note" role="note">
    {children}
  </p>
);

const SiteTabs = ({
  siteId,
  view,
  access,
}: {
  readonly siteId: SiteId;
  readonly view: SiteView;
  readonly access: ReadonlySet<SiteCapability> | null;
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
        {SITE_TABS.filter(
          (tab) => tab.needs === undefined || access === null || access.has(tab.needs),
        ).map((tab) => (
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
  const user = useSession();
  const access =
    site !== null ? siteAccess(site, user) : loaded.status === 'error' ? FULL_ACCESS : null;
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
      <SiteTabs siteId={siteId} view={view} access={access} />
      <SiteAccessContext.Provider value={access}>
        <SiteSavedContext.Provider value={setSaved}>{children}</SiteSavedContext.Provider>
      </SiteAccessContext.Provider>
    </div>
  );
};
