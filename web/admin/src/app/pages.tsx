/**
 * The admin page table: one row per controller under
 * `application/src/Controller/Admin/*.php` of omeka-s at 5e413f0, except the
 * vocabulary family, whose three controllers share the Standards page. The nav
 * groups follow the work, not the upstream admin navigation.
 *
 * Every `element` is a `React.lazy` component, so a route's module is fetched
 * the first time that route opens and the entry bundle carries the shell and
 * this table alone. The `AdminPageId` union and the row fields are what the nav
 * and the e2e table reader read, so they keep the shape both depend on.
 */
import { lazy } from 'react';
import type { ComponentType } from 'react';
import { api } from '../api/client';
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
  | 'standards'
  | 'users'
  | 'modules'
  | 'jobs'
  | 'settings'
  | 'assets'
  | 'system-info'
  | 'columns'
  | 'query';

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
   * Whether the nav offers the page, asked once per signed-in user. A page
   * without it is always offered; its route works either way.
   */
  readonly offered?: () => Promise<boolean>;
}

export type PageGroupId = 'content' | 'describe' | 'admin';

export interface PageGroup {
  readonly id: PageGroupId;
  readonly titleKey: MessageKey;
  readonly pages: readonly AdminPageId[];
  /** A labelled run of links after the group's own, such as Describe's Advanced. */
  readonly sub?: { readonly titleKey: MessageKey; readonly pages: readonly AdminPageId[] };
}

/**
 * The nav, grouped by the work rather than by the tables: Describe is where a
 * collection's item types and fields are shaped, and the raw vocabulary layer
 * sits under its Advanced heading.
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
    pages: ['resource-templates', 'term-lists', 'fields'],
    sub: { titleKey: 'nav.advanced', pages: ['standards'] },
  },
  {
    id: 'admin',
    titleKey: 'group.admin',
    pages: ['users', 'modules', 'jobs', 'settings', 'assets', 'system-info', 'columns', 'query'],
  },
];

export const PAGES: Readonly<Record<AdminPageId, AdminPage>> = {
  dashboard: {
    titleKey: 'page.dashboard',
    upstream: 'Admin/IndexController.php browseAction',
    element: lazy(() =>
      import('../pages/dashboard/page').then((module) => ({ default: module.DashboardPage })),
    ),
  },
  items: {
    titleKey: 'page.items',
    upstream: 'Admin/ItemController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/items/page').then((module) => ({ default: module.ItemsPage })),
    ),
  },
  'item-sets': {
    titleKey: 'page.itemSets',
    upstream: 'Admin/ItemSetController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/item-sets/page').then((module) => ({ default: module.ItemSetsPage })),
    ),
  },
  media: {
    titleKey: 'page.media',
    upstream: 'Admin/MediaController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/media/page').then((module) => ({ default: module.MediaPage })),
    ),
  },
  import: {
    titleKey: 'page.import',
    upstream: 'Romeka: GET /api/romeka/archive, /api/romeka/imports (docs/archive-import.md)',
    element: lazy(() =>
      import('../pages/import/page').then((module) => ({ default: module.ImportPage })),
    ),
    // A step's file table lays a column per pattern group.
    wide: (rest) => rest[0] !== undefined,
    // Hidden while the server has no `ROMEKA_ARCHIVE_ROOT`; the client loads with the page.
    offered: () => import('../api/imports').then((module) => module.archiveEnabled(api)),
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
  },
  'resource-templates': {
    titleKey: 'page.itemTypes',
    upstream: 'Admin/ResourceTemplateController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/item-types/page').then((module) => ({ default: module.ItemTypesPage })),
    ),
    // The item type editor lays a live preview beside its fields.
    wide: (rest) => rest[0] !== undefined,
  },
  fields: {
    titleKey: 'page.fields',
    upstream: 'Admin/PropertyController.php browseAction',
    element: lazy(() =>
      import('../pages/fields/page').then((module) => ({ default: module.FieldsPage })),
    ),
  },
  'term-lists': {
    titleKey: 'page.termLists',
    upstream: 'CustomVocab Controller/Admin/IndexController.php browseAction addAction editAction',
    element: lazy(() =>
      import('../pages/term-lists/page').then((module) => ({ default: module.TermListsPage })),
    ),
  },
  standards: {
    titleKey: 'page.standards',
    upstream:
      'Admin/VocabularyController.php, Admin/PropertyController.php, Admin/ResourceClassController.php',
    element: lazy(() =>
      import('../pages/standards/page').then((module) => ({ default: module.StandardsPage })),
    ),
  },
  users: {
    titleKey: 'page.users',
    upstream: 'Admin/UserController.php browseAction',
    element: lazy(() =>
      import('../pages/users/page').then((module) => ({ default: module.UsersPage })),
    ),
  },
  modules: {
    titleKey: 'page.modules',
    upstream: 'Admin/ModuleController.php browseAction',
    element: lazy(() =>
      import('../pages/modules/page').then((module) => ({ default: module.ModulesPage })),
    ),
  },
  jobs: {
    titleKey: 'page.jobs',
    upstream: 'Admin/JobController.php browseAction',
    element: lazy(() =>
      import('../pages/jobs/page').then((module) => ({ default: module.JobsPage })),
    ),
  },
  settings: {
    titleKey: 'page.settings',
    upstream: 'Admin/SettingController.php browseAction',
    element: lazy(() =>
      import('../pages/settings/page').then((module) => ({ default: module.SettingsPage })),
    ),
  },
  assets: {
    titleKey: 'page.assets',
    upstream: 'Admin/AssetController.php browseAction',
    element: lazy(() =>
      import('../pages/assets/page').then((module) => ({ default: module.AssetsPage })),
    ),
  },
  'system-info': {
    titleKey: 'page.systemInfo',
    upstream: 'Admin/SystemInfoController.php browseAction',
    element: lazy(() =>
      import('../pages/system-info/page').then((module) => ({ default: module.SystemInfoPage })),
    ),
  },
  columns: {
    titleKey: 'page.columns',
    upstream: 'Admin/ColumnsController.php columnListAction',
    element: lazy(() =>
      import('../pages/columns/page').then((module) => ({ default: module.ColumnsPage })),
    ),
  },
  query: {
    titleKey: 'page.query',
    upstream: 'Admin/QueryController.php sidebarEditAction',
    element: lazy(() =>
      import('../pages/query/page').then((module) => ({ default: module.QueryPage })),
    ),
  },
};

export const PAGE_IDS: ReadonlySet<string> = new Set(Object.keys(PAGES));

export const isAdminPageId = (value: string): value is AdminPageId => PAGE_IDS.has(value);
