import { Component, lazy, Suspense, useEffect, useRef } from 'react';
import type { ReactElement, ReactNode, RefObject } from 'react';
import { api } from '../api/client';
import { currentUser, setSession } from '../api/session';
import { LOCALES, parseLocale, setLocale, useLocale, useTranslator } from '../i18n';
import { AuthBadge, useSession } from './auth';
import { PAGE_GROUPS, PAGES } from './pages';
import type { AdminPageId } from './pages';
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

const RouteFailed = (): ReactElement => {
  const { t } = useTranslator();
  return (
    <div role="alert">
      <p>{t('shell.loadFailed')}</p>
      <button
        type="button"
        onClick={() => {
          window.location.reload();
        }}
      >
        {t('shell.reload')}
      </button>
    </div>
  );
};

/**
 * Catches a route whose chunk or message table failed to load, or that threw
 * while rendering, so the shell and its nav stay usable. React keeps a failed
 * lazy import failed, so the way back is a reload. The shell keys it by page,
 * so another nav link renders that page afresh.
 */
class RouteBoundary extends Component<
  { readonly children: ReactNode },
  { readonly failed: boolean }
> {
  override state = { failed: false };

  static getDerivedStateFromError(): { readonly failed: boolean } {
    return { failed: true };
  }

  override componentDidCatch(error: unknown): void {
    console.error(error);
  }

  override render(): ReactNode {
    return this.state.failed ? <RouteFailed /> : this.props.children;
  }
}

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
 * the choice through the locale store, so a reload keeps it. The page switches
 * once the chosen catalogue has loaded. The stored value
 * is re-narrowed by `parseLocale`, the same boundary the store reads through.
 *
 * A signed-in user's choice is also their `locale` user setting, so it follows
 * them to another browser. That write is best-effort: the switch has already
 * happened here, so a refused write is logged and nothing else.
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
          // A catalogue that fails to load leaves the current language, and
          // the select, where they were.
          const chosen = parseLocale(event.target.value);
          setLocale(chosen)
            .then(() => {
              const user = currentUser();
              return user === null ? undefined : api.updateUserSettings(user.id, { locale: chosen });
            })
            .catch((error: unknown) => {
              console.error(error);
            });
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

const NavLink = ({ id, active }: { readonly id: AdminPageId; readonly active: boolean }): ReactElement | null => {
  const { t } = useTranslator();
  const user = useSession();
  const { offered } = PAGES[id];
  // A refused or failed probe hides the link, as an unconfigured feature does.
  const probe = useAsync(() => (offered === undefined ? Promise.resolve(true) : offered()), [offered, user?.id]);
  if (offered !== undefined && !(probe.status === 'ready' && probe.data)) {
    return null;
  }
  return (
    <li>
      <a className="nav-link" href={`#/${id}`} aria-current={active ? 'page' : undefined}>
        {t(PAGES[id].titleKey)}
      </a>
    </li>
  );
};

const NavLinks = ({
  pages,
  activeId,
}: {
  readonly pages: readonly AdminPageId[];
  readonly activeId: AdminPageId | null;
}): ReactElement[] => pages.map((id) => <NavLink key={id} id={id} active={activeId === id} />);

/** The gap left before a link the strip scrolls to, so it does not touch the edge. */
const NAV_STRIP_INSET = 16;

/**
 * Bring the active link into the phone nav strip, which scrolls sideways and
 * would otherwise open at its start whatever the route. The wide rail does not
 * scroll, so the guard leaves it alone.
 */
const bringActiveLinkIntoView = (nav: HTMLElement): void => {
  const link = nav.querySelector('[aria-current="page"]');
  if (!(link instanceof HTMLElement) || nav.scrollWidth <= nav.clientWidth) {
    return;
  }
  const navBox = nav.getBoundingClientRect();
  const linkBox = link.getBoundingClientRect();
  if (linkBox.left < navBox.left || linkBox.right > navBox.right) {
    // A link under a sub-heading keeps its heading in view: "Advanced Standards".
    const start = (link.closest('.nav-sub') ?? link).getBoundingClientRect().left;
    nav.scrollLeft += start - navBox.left - NAV_STRIP_INSET;
  }
};

const useActiveLinkInView = (activeId: AdminPageId | null): RefObject<HTMLElement | null> => {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    const nav = ref.current;
    if (nav === null) {
      return undefined;
    }
    bringActiveLinkIntoView(nav);
    // A link shown only once its probe answers (Import) can land before the
    // active one after this first pass and push it out of the strip again.
    const observer = new MutationObserver(() => {
      bringActiveLinkIntoView(nav);
    });
    observer.observe(nav, { childList: true, subtree: true });
    return () => {
      observer.disconnect();
    };
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
        <RouteBoundary key={authRoute.kind}>
          <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
            {authRoute.kind === 'password' ? (
              <PasswordPage passwordKey={authRoute.key} />
            ) : (
              <ForgotPasswordPage />
            )}
          </Suspense>
        </RouteBoundary>
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
            <RouteBoundary key={activeId}>
              <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
                <page.element param={route === null ? null : route.param} rest={route?.rest ?? []} />
              </Suspense>
            </RouteBoundary>
          </>
        )}
      </main>
    </div>
  );
};
