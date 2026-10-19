/**
 * The admin page table: one row per controller under
 * `application/src/Controller/Admin/*.php` of omeka-s at 5e413f0, grouped the
 * way the upstream admin navigation presents them.
 *
 * Every `element` is a `React.lazy` component, so a route's module is fetched
 * the first time that route opens and the entry bundle carries the shell and
 * this table alone. The `AdminPageId` union and the row fields are what the nav
 * and the e2e table reader read, so they keep the shape both depend on.
 */
import { lazy } from 'react';
import type { ComponentType } from 'react';
import type { MessageKey } from '../i18n';
import type { PageProps } from './router';

export type AdminPageId =
  | 'dashboard'
  | 'items'
  | 'item-sets'
  | 'media'
  | 'sites'
  | 'vocabularies'
  | 'properties'
  | 'resource-classes'
  | 'resource-templates'
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
}

export type PageGroupId = 'content' | 'vocabulary' | 'admin';

export interface PageGroup {
  readonly id: PageGroupId;
  readonly titleKey: MessageKey;
  readonly pages: readonly AdminPageId[];
}

export const PAGE_GROUPS: readonly PageGroup[] = [
  {
    id: 'content',
    titleKey: 'group.content',
    pages: ['dashboard', 'items', 'item-sets', 'media', 'sites'],
  },
  {
    id: 'vocabulary',
    titleKey: 'group.vocabulary',
    pages: ['vocabularies', 'properties', 'resource-classes', 'resource-templates'],
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
  vocabularies: {
    titleKey: 'page.vocabularies',
    upstream:
      'Admin/VocabularyController.php browseAction showDetailsAction propertiesAction classesAction',
    element: lazy(() =>
      import('../pages/vocabularies/page').then((module) => ({ default: module.VocabulariesPage })),
    ),
  },
  properties: {
    titleKey: 'page.properties',
    upstream: 'Admin/PropertyController.php showDetailsAction',
    element: lazy(() =>
      import('../pages/properties/page').then((module) => ({ default: module.PropertiesPage })),
    ),
  },
  'resource-classes': {
    titleKey: 'page.resourceClasses',
    upstream: 'Admin/ResourceClassController.php showDetailsAction',
    element: lazy(() =>
      import('../pages/resource-classes/page').then((module) => ({
        default: module.ResourceClassesPage,
      })),
    ),
  },
  'resource-templates': {
    titleKey: 'page.resourceTemplates',
    upstream: 'Admin/ResourceTemplateController.php browseAction showAction',
    element: lazy(() =>
      import('../pages/resource-templates/page').then((module) => ({
        default: module.ResourceTemplatesPage,
      })),
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
