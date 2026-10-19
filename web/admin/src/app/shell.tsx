import { lazy, Suspense, useEffect } from 'react';
import type { ReactElement } from 'react';
import { api } from '../api/client';
import { currentUser, setSession } from '../api/session';
import { LOCALES, parseLocale, setLocale, useLocale, useTranslator } from '../i18n';
import { AuthBadge } from './auth';
import { PAGE_GROUPS, PAGES } from './pages';
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
              return user === null ? undefined : api.updateUserLocale(user.id, chosen);
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
      <nav className="app-nav" aria-label={t('shell.navLabel')}>
        {PAGE_GROUPS.map((group) => (
          <section key={group.id} className="nav-group">
            <h2 className="nav-group-title">{t(group.titleKey)}</h2>
            <ul className="nav-list">
              {group.pages.map((id) => (
                <li key={id}>
                  <a
                    className="nav-link"
                    href={`#/${id}`}
                    aria-current={activeId === id ? 'page' : undefined}
                  >
                    {t(PAGES[id].titleKey)}
                  </a>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </nav>
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
