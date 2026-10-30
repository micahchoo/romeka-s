/**
 * The admin page table: one row per controller under
 * `application/src/Controller/Admin/*.php` of omeka-s at 5e413f0, except two
 * families that share one page: the vocabulary controllers share Standards, and
 * System information, Modules and Query share System. `ColumnsController` has no
 * row (see `pages/system/page.tsx`). The nav groups follow the work, not the
 * upstream admin navigation.
 *
 * Every `element` is a `React.lazy` component, so a route's module is fetched
 * the first time that route opens and the entry bundle carries the shell and
 * this table alone. The `AdminPageId` union and the row fields are what the nav
 * and the e2e table reader read, so they keep the shape both depend on.
 */
import { lazy } from 'react';
import type { ComponentType } from 'react';
import { USER_ROLES } from '../api/types';
import type { UserRole } from '../api/types';
import type { MessageKey } from '../i18n';
import type { PageProps } from './router';

export type AdminPageId =
  | 'dashboard'
  | 'items'
  | 'item-sets'
  | 'media'
  | 'import'
  | 'sites'
  | 'resource-templates'
  | 'term-lists'
  | 'fields'
  | 'values'
  | 'standards'
  | 'users'
  | 'jobs'
  | 'settings'
  | 'assets'
  | 'system';

export interface AdminPage {
  /** The catalogue key the nav and the heading read. */
  readonly titleKey: MessageKey;
  /** The upstream controller action this row stands for. */
  readonly upstream: string;
  readonly element: ComponentType<PageProps>;
  /** Whether the route's content takes the full width instead of the reading
   * width, read from the hash segments after the page id. */
  readonly wide?: (rest: readonly string[]) => boolean;
  /**
   * The roles the nav offers the page to: the roles the server lets read what
   * the page shows. The route opens for anyone, and the server still answers
   * each request for the caller.
   */
  readonly offered: readonly UserRole[];
}

/** Every role: the page reads only what the API serves any signed-in user. */
const EVERY_ROLE: readonly UserRole[] = USER_ROLES;

/** `Actor::is_privileged` in `omeka-api`: the roles that read jobs and modules. */
const PRIVILEGED: readonly UserRole[] = ['global_admin', 'site_admin', 'editor', 'reviewer'];

/**
 * Whether the nav offers `page` to a caller with `role`. A caller signed in by
 * an API key alone has no role the admin knows, so only the pages every role
 * may use are offered to them.
 */
export const offeredTo = (page: AdminPage, role: string | null): boolean =>
  role === null
    ? page.offered.length === USER_ROLES.length
    : page.offered.some((offered) => offered === role);

export type PageGroupId = 'content' | 'describe' | 'admin';

export interface PageGroup {
  readonly id: PageGroupId;
  readonly titleKey: MessageKey;
  readonly pages: readonly AdminPageId[];
  /** A labelled run of links after the group's own, such as an Advanced heading. */
  readonly sub?: { readonly titleKey: MessageKey; readonly pages: readonly AdminPageId[] };
}

/**
 * The nav, grouped by the work rather than by the tables: Describe is where a
 * collection's item types and fields are shaped, and the raw vocabulary layer
 * sits under its Advanced heading. Admin's Advanced heading holds System, the
 * installation facts and tools few people open.
 */
export const PAGE_GROUPS: readonly PageGroup[] = [
  {
    id: 'content',
    titleKey: 'group.content',
    pages: ['dashboard', 'items', 'item-sets', 'media', 'import', 'sites'],
  },
  {
    id: 'describe',
    titleKey: 'group.describe',
    pages: ['resource-templates', 'term-lists', 'fields', 'values'],
    sub: { titleKey: 'nav.advanced', pages: ['standards'] },
  },
  {
    id: 'admin',
    titleKey: 'group.admin',
    pages: ['users', 'jobs', 'settings', 'assets'],
    sub: { titleKey: 'nav.advanced', pages: ['system'] },
  },
];

export const PAGES: Readonly<Record<AdminPageId, AdminPage>> = {
  dashboard: {
    titleKey: 'page.dashboard',
    upstream: 'Admin/IndexController.php browseAction',
    element: lazy(() =>
      import('../pages/dashboard/page').then((module) => ({ default: module.DashboardPage })),
    ),
    offered: EVERY_ROLE,
  },
  items: {
    titleKey: 'page.items',
    upstream: 'Admin/ItemController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/items/page').then((module) => ({ default: module.ItemsPage })),
    ),
    offered: EVERY_ROLE,
  },
  'item-sets': {
    titleKey: 'page.itemSets',
    upstream: 'Admin/ItemSetController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/item-sets/page').then((module) => ({ default: module.ItemSetsPage })),
    ),
    offered: EVERY_ROLE,
  },
  media: {
    titleKey: 'page.media',
    upstream: 'Admin/MediaController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/media/page').then((module) => ({ default: module.MediaPage })),
    ),
    offered: EVERY_ROLE,
  },
  import: {
    titleKey: 'page.import',
    upstream: 'Romeka: GET /api/romeka/archive, /api/romeka/imports (docs/import-v2.md)',
    element: lazy(() =>
      import('../pages/import/page').then((module) => ({ default: module.ImportPage })),
    ),
    // One import's screens hold a side index and wide tables of records.
    wide: (rest) => rest[0] !== undefined,
    // `imports::wire::admin` in `omeka-server` answers the two admin roles.
    offered: ['global_admin', 'site_admin'],
  },
  sites: {
    titleKey: 'page.sites',
    upstream: 'SiteAdmin/IndexController.php indexAction addAction editAction',
    element: lazy(() =>
      import('../pages/sites/page').then((module) => ({ default: module.SitesPage })),
    ),
    // The page editor (`#/sites/<id>/pages/...`) and the Theme page
    // (`#/sites/<id>/theme`) lay a preview beside their controls.
    wide: (rest) => rest[1] === 'pages' || rest[1] === 'theme',
    offered: EVERY_ROLE,
  },
  'resource-templates': {
    titleKey: 'page.itemTypes',
    upstream: 'Admin/ResourceTemplateController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/item-types/page').then((module) => ({ default: module.ItemTypesPage })),
    ),
    // The item type editor lays a live preview beside its fields.
    wide: (rest) => rest[0] !== undefined,
    offered: EVERY_ROLE,
  },
  fields: {
    titleKey: 'page.fields',
    upstream: 'Admin/PropertyController.php browseAction',
    element: lazy(() =>
      import('../pages/fields/page').then((module) => ({ default: module.FieldsPage })),
    ),
    offered: EVERY_ROLE,
  },
  values: {
    titleKey: 'page.values',
    upstream: 'Romeka: GET /api/romeka/field-values/{property}, POST /api/romeka/field-values/move',
    element: lazy(() =>
      import('../pages/fields/values-page').then((module) => ({ default: module.ValuesPage })),
    ),
    offered: EVERY_ROLE,
  },
  'term-lists': {
    titleKey: 'page.termLists',
    upstream: 'CustomVocab Controller/Admin/IndexController.php browseAction addAction editAction',
    element: lazy(() =>
      import('../pages/term-lists/page').then((module) => ({ default: module.TermListsPage })),
    ),
    offered: EVERY_ROLE,
  },
  standards: {
    titleKey: 'page.standards',
    upstream:
      'Admin/VocabularyController.php, Admin/PropertyController.php, Admin/ResourceClassController.php',
    element: lazy(() =>
      import('../pages/standards/page').then((module) => ({ default: module.StandardsPage })),
    ),
    offered: EVERY_ROLE,
  },
  users: {
    titleKey: 'page.users',
    upstream: 'Admin/UserController.php browseAction',
    element: lazy(() =>
      import('../pages/users/page').then((module) => ({ default: module.UsersPage })),
    ),
    offered: EVERY_ROLE,
  },
  jobs: {
    titleKey: 'page.jobs',
    upstream: 'Admin/JobController.php browseAction',
    element: lazy(() =>
      import('../pages/jobs/page').then((module) => ({ default: module.JobsPage })),
    ),
    // `Api::allow` in `omeka-api` reads `jobs` for the privileged roles only.
    offered: PRIVILEGED,
  },
  settings: {
    titleKey: 'page.settings',
    upstream: 'Admin/SettingController.php browseAction',
    element: lazy(() =>
      import('../pages/settings/page').then((module) => ({ default: module.SettingsPage })),
    ),
    // `settings::authorize_global` in `omeka-api`.
    offered: ['global_admin'],
  },
  assets: {
    titleKey: 'page.assets',
    upstream: 'Admin/AssetController.php browseAction',
    element: lazy(() =>
      import('../pages/assets/page').then((module) => ({ default: module.AssetsPage })),
    ),
    offered: EVERY_ROLE,
  },
  system: {
    titleKey: 'page.system',
    upstream:
      'Admin/SystemInfoController.php browseAction, Admin/ModuleController.php browseAction, Admin/QueryController.php sidebarEditAction',
    element: lazy(() =>
      import('../pages/system/page').then((module) => ({ default: module.SystemPage })),
    ),
    // `Api::allow` in `omeka-api` reads `modules` for the privileged roles only.
    offered: PRIVILEGED,
  },
};

export const PAGE_IDS: ReadonlySet<string> = new Set(Object.keys(PAGES));

export const isAdminPageId = (value: string): value is AdminPageId => PAGE_IDS.has(value);
