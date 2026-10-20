/**
 * What the signed-in user may do to one site, as the server decides it
 * (`AclFactory::addRulesForSites`; `crates/omeka-api/src/read/mod.rs`
 * `authorize_site_update`, `authorize_site_add_page`, `authorize_site_page`,
 * and `write/sites.rs::delete`).
 *
 * A caller's standing on a site is one of five; each standing grants a fixed
 * set of capabilities. The admin hides or explains a control whose capability
 * is missing, so a Creator is never offered a write that answers 403.
 */
import type { Site, User } from '../../api/types';

/** A write the site screens offer. */
export type SiteCapability =
  /** Update the site itself: homepage, menu, items, users, theme, settings. */
  | 'changeSite'
  /** Add, edit and delete the site's pages. */
  | 'editPages'
  /** Remove the whole site. */
  | 'deleteSite';

/** Where a caller stands on one site. */
type SiteStanding = 'owner' | 'manager' | 'creator' | 'viewer' | 'none';

const GRANTS: Readonly<Record<SiteStanding, readonly SiteCapability[]>> = {
  owner: ['changeSite', 'editPages', 'deleteSite'],
  manager: ['changeSite', 'editPages'],
  creator: ['editPages'],
  viewer: [],
  none: [],
};

/** The global roles the server lets do anything (`access::is_admin_role`). */
const ADMIN_ROLES: readonly string[] = ['global_admin', 'site_admin'];

/** `o:site_permission` roles, upstream's `admin`, `editor`, `viewer` (Manager, Creator, Viewer). */
const SITE_ROLE_STANDING: Readonly<Record<string, SiteStanding>> = {
  admin: 'manager',
  editor: 'creator',
};

const standing = (site: Site, user: User): SiteStanding => {
  if (ADMIN_ROLES.includes(user.role) || site.owner?.id === user.id) {
    return 'owner';
  }
  const permission = site.sitePermissions.find((entry) => entry.user.id === user.id);
  if (permission === undefined) {
    return 'none';
  }
  // `HasSitePermissionAssertion` reads any other role string as a viewer.
  return SITE_ROLE_STANDING[permission.role] ?? 'viewer';
};

/** Every capability: what a caller with no known session is offered. */
export const FULL_ACCESS: ReadonlySet<SiteCapability> = new Set(GRANTS.owner);

/**
 * What `user` may do to `site`. With no session known (an API key alone, or
 * the session probe still out) every control is offered and the server's rule
 * decides the write, as the item form's Sites control does.
 */
export const siteAccess = (site: Site, user: User | null): ReadonlySet<SiteCapability> =>
  user === null ? FULL_ACCESS : new Set(GRANTS[standing(site, user)]);
