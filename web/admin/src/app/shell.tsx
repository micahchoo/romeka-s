import { lazy, Suspense, useEffect, useRef } from 'react';
import type { ReactElement, RefObject } from 'react';
import { api } from '../api/client';
import { setSession } from '../api/session';
import { LOCALES, parseLocale, setLocale, useLocale, useTranslator } from '../i18n';
import { AuthBadge } from './auth';
import { isAdminPageId, PAGE_GROUPS, PAGES } from './pages';
import type { AdminPageId, NavRoute } from './pages';
import { parseAuthRoute, parseHash, useHash } from './router';
import { useAsync } from './useAsync';

/**
 * The set-password and forgot surfaces are separate chunks, the way every admin
 * route is, because the shell renders them only for a caller who followed a
 * link and not for anyone signed in.
 */
const PasswordPage = lazy(() =>
  import('./password').then((module) => ({ default: module.PasswordPage })),
);
const ForgotPasswordPage = lazy(() =>
  import('./forgot').then((module) => ({ default: module.ForgotPasswordPage })),
);

/**
 * The Omeka S version the client reads from the `Omeka-S-Version` response
 * header. Nothing renders while the request is in flight or when it fails, so
 * an unreachable API never blocks the page.
 */
const VersionBadge = (): ReactElement | null => {
  const state = useAsync(() => api.getApiInfo(), []);
  if (state.status !== 'ready' || state.data.version === null) {
    return null;
  }
  return <span className="app-version">{state.data.version}</span>;
};

/**
 * The locale switch. It offers every shipped locale by its own name and stores
 * the choice through the locale store, so a reload keeps it. The stored value
 * is re-narrowed by `parseLocale`, the same boundary the store reads through.
 */
const LocaleSwitch = (): ReactElement => {
  const locale = useLocale();
  const { t } = useTranslator();
  return (
    <label className="locale-switch">
      <span className="locale-switch-label">{t('locale.label')}</span>
      <select
        value={locale}
        onChange={(event) => {
          setLocale(parseLocale(event.target.value));
        }}
      >
        {LOCALES.map((option) => (
          <option key={option} value={option}>
            {t(`locale.${option}`)}
          </option>
        ))}
      </select>
    </label>
  );
};

/**
 * Ask the server who is signed in, once, on load.
 *
 * The session cookie is `httpOnly`, so storage holds no trace of it and this one
 * request is the only way the SPA learns whether a reload kept the session.
 * Nothing waits on the answer: the header shows the key's state until the user
 * arrives, and an unreachable API leaves the caller signed out rather than
 * blocking the page.
 */
const useSessionProbe = (): void => {
  useEffect(() => {
    let cancelled = false;
    api.getMe().then(
      (user) => {
        if (!cancelled) {
          setSession(user);
        }
      },
      () => {
        if (!cancelled) {
          setSession(null);
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);
};

const NavLinks = ({
  pages,
  activeId,
}: {
  readonly pages: readonly NavRoute[];
  readonly activeId: AdminPageId | null;
}): ReactElement[] => {
  const { t } = useTranslator();
  return pages.filter(isAdminPageId).map((id) => (
    <li key={id}>
      <a className="nav-link" href={`#/${id}`} aria-current={activeId === id ? 'page' : undefined}>
        {t(PAGES[id].titleKey)}
      </a>
    </li>
  ));
};

/** The gap left before a link the strip scrolls to, so it does not touch the edge. */
const NAV_STRIP_INSET = 16;

/**
 * Bring the active link into the phone nav strip, which scrolls sideways and
 * would otherwise open at its start whatever the route. The wide rail does not
 * scroll, so the guard leaves it alone.
 */
const useActiveLinkInView = (activeId: AdminPageId | null): RefObject<HTMLElement | null> => {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    const nav = ref.current;
    const link = nav?.querySelector('[aria-current="page"]');
    if (nav === null || !(link instanceof HTMLElement) || nav.scrollWidth <= nav.clientWidth) {
      return;
    }
    const navBox = nav.getBoundingClientRect();
    const linkBox = link.getBoundingClientRect();
    if (linkBox.left < navBox.left || linkBox.right > navBox.right) {
      // A link under a sub-heading keeps its heading in view: "Advanced Standards".
      const start = (link.closest('.nav-sub') ?? link).getBoundingClientRect().left;
      nav.scrollLeft += start - navBox.left - NAV_STRIP_INSET;
    }
  }, [activeId]);
  return ref;
};

const AdminNav = ({ activeId }: { readonly activeId: AdminPageId | null }): ReactElement => {
  const { t } = useTranslator();
  const ref = useActiveLinkInView(activeId);
  return (
    <nav className="app-nav" aria-label={t('shell.navLabel')} ref={ref}>
      {PAGE_GROUPS.map((group) => (
        <section key={group.id} className="nav-group">
          <h2 className="nav-group-title">{t(group.titleKey)}</h2>
          <ul className="nav-list">
            <NavLinks pages={group.pages} activeId={activeId} />
            {group.sub === undefined ? null : (
              <li className="nav-sub">
                <h3 className="nav-sub-title">{t(group.sub.titleKey)}</h3>
                <ul className="nav-list">
                  <NavLinks pages={group.sub.pages} activeId={activeId} />
                </ul>
              </li>
            )}
          </ul>
        </section>
      ))}
    </nav>
  );
};

/** The layout a set-password or forgot link opens: the header, and no admin nav. */
const AuthLayout = ({ children }: { readonly children: ReactElement }): ReactElement => {
  const { t } = useTranslator();
  return (
    <div className="shell">
      <header className="app-header">
        <span className="app-title">{t('app.title')}</span>
        <AuthBadge />
      </header>
      <main className="app-main">{children}</main>
    </div>
  );
};

export const Shell = (): ReactElement => {
  const { t } = useTranslator();
  const hash = useHash();
  useSessionProbe();
  const authRoute = parseAuthRoute(hash, window.location.search);

  if (authRoute !== null) {
    return (
      <AuthLayout>
        <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
          {authRoute.kind === 'password' ? (
            <PasswordPage passwordKey={authRoute.key} />
          ) : (
            <ForgotPasswordPage />
          )}
        </Suspense>
      </AuthLayout>
    );
  }

  const route = parseHash(hash);
  const activeId = route === null ? null : route.page;
  const page = route === null ? null : PAGES[route.page];

  return (
    <div className="shell">
      <header className="app-header">
        <span className="app-title">{t('app.title')}</span>
        <VersionBadge />
        <LocaleSwitch />
        <AuthBadge />
      </header>
      <AdminNav activeId={activeId} />
      <main
        className={
          page !== null && route !== null && page.wide?.(route.rest) === true
            ? 'app-main app-main--wide'
            : 'app-main'
        }
      >
        {page === null ? (
          <p role="alert">{t('shell.noPage')}</p>
        ) : (
          <>
            <h1>{t(page.titleKey)}</h1>
            {/* The page module is fetched on the first visit to this route. The
                heading comes from the table, so it paints before the chunk does. */}
            <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
              <page.element param={route === null ? null : route.param} rest={route?.rest ?? []} />
            </Suspense>
          </>
        )}
      </main>
    </div>
  );
};
