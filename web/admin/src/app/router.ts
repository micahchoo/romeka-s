/**
 * Hash routing for the admin SPA. Routes are `#/<page-id>` with one optional
 * fragment parameter (`#/items/12`), so the Rust server needs no rewrite rules.
 *
 * Two surfaces sit outside the admin page table: set-password and forgot. They
 * render without the admin navigation, because whoever opens them cannot sign in
 * yet, and a mail link names the one-time key that set-password reads.
 */
import { useEffect, useState } from 'react';
import { isAdminPageId } from './pages';
import type { AdminPageId } from './pages';

export interface RouteMatch {
  readonly page: AdminPageId;
  readonly param: string | null;
  /** Every fragment segment after the page id, for nested routes. */
  readonly rest: readonly string[];
}

/** Props every page component receives from the router. */
export interface PageProps {
  readonly param: string | null;
  /** Present on nested routes such as `#/sites/1/pages/2`. */
  readonly rest?: readonly string[];
}

export const DEFAULT_PAGE: AdminPageId = 'dashboard';

/**
 * Routes that moved, each with the page and the leading segments that now
 * serve it. Upstream's vocabulary family routes open a Standards tab, so a
 * bookmark or a link such as `#/properties/12` still lands on that property.
 */
const MOVED: ReadonlyMap<string, readonly [AdminPageId, string]> = new Map([
  ['vocabularies', ['standards', 'vocabularies']],
  ['properties', ['standards', 'properties']],
  ['resource-classes', ['standards', 'classes']],
]);

export const parseHash = (hash: string): RouteMatch | null => {
  const trimmed = hash.startsWith('#') ? hash.slice(1) : hash;
  const segments = trimmed.split('/').filter((segment) => segment !== '');
  const slug = segments[0];
  if (slug === undefined) {
    return { page: DEFAULT_PAGE, param: null, rest: [] };
  }
  const moved = MOVED.get(slug);
  if (moved !== undefined) {
    const rest = [moved[1], ...segments.slice(1)];
    return { page: moved[0], param: moved[1], rest };
  }
  if (!isAdminPageId(slug)) {
    return null;
  }
  const rest = segments.slice(1);
  return { page: slug, param: rest[0] ?? null, rest };
};

/** The current hash, tracked so a route change re-renders the shell. */
export const useHash = (): string => {
  const [hash, setHash] = useState(() => window.location.hash);
  useEffect(() => {
    const onChange = (): void => {
      setHash(window.location.hash);
    };
    window.addEventListener('hashchange', onChange);
    return () => {
      window.removeEventListener('hashchange', onChange);
    };
  }, []);
  return hash;
};

/**
 * The set-password and forgot surfaces, or `null` for an admin route.
 *
 * A one-time key reaches set-password through the URL alone, and the server
 * builds the mail link, so the page accepts each shape that link can take: the
 * upstream action's slug (`#/create-password?key=...`), the port's own
 * (`#/password/<key>`), and a bare `?key=...` on the document, which is what a
 * link against the app root produces. A `#/forgot` fragment is the request form.
 */
export type AuthRoute =
  | { readonly kind: 'password'; readonly key: string | null }
  | { readonly kind: 'forgot' };

const PASSWORD_SLUGS: ReadonlySet<string> = new Set(['password', 'create-password']);

/** The hash's path segments and query, split at the first `?`. */
const splitHash = (hash: string): { segments: readonly string[]; query: URLSearchParams } => {
  const trimmed = hash.startsWith('#') ? hash.slice(1) : hash;
  const cut = trimmed.indexOf('?');
  const path = cut === -1 ? trimmed : trimmed.slice(0, cut);
  return {
    segments: path.split('/').filter((segment) => segment !== ''),
    query: new URLSearchParams(cut === -1 ? '' : trimmed.slice(cut + 1)),
  };
};

/**
 * Where a sign-in sends the reader back to: the `redirect` the public site's
 * Log in link carries (`/login?redirect=/s/<site>/...`). Only a path on this
 * origin passes: it starts with one `/`, never `//host`, and holds printable
 * ASCII with no backslash, which some browsers read as a slash. The server's
 * Logout applies the same rule (`omeka-server/src/accounts.rs`).
 */
export const signInReturnPath = (search: string): string | null => {
  const target = new URLSearchParams(search).get('redirect');
  if (target === null || !/^\/(?!\/)[\x21-\x7e]*$/.test(target) || target.includes('\\')) {
    return null;
  }
  return target;
};

export const parseAuthRoute = (hash: string, search: string): AuthRoute | null => {
  const { segments, query } = splitHash(hash);
  const slug = segments[0];
  const fromSearch = new URLSearchParams(search).get('key');
  if (slug !== undefined && PASSWORD_SLUGS.has(slug)) {
    return { kind: 'password', key: query.get('key') ?? segments[1] ?? fromSearch };
  }
  if (slug === 'forgot') {
    return { kind: 'forgot' };
  }
  return fromSearch === null ? null : { kind: 'password', key: fromSearch };
};
