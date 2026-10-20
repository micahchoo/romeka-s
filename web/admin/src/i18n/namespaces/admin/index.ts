/**
 * The admin page messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `AdminMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'jobs.status.starting': 'Starting',
  'jobs.status.stopping': 'Stopping',
  'jobs.status.in_progress': 'In Progress',
  'jobs.status.completed': 'Completed',
  'jobs.status.stopped': 'Stopped',
  'jobs.status.error': 'Error',
  'page.resourceTemplates': 'Resource templates',
  'common.sameOrigin': '[Same origin]',
  'action.preview': 'Preview',
  // Loading states that are not a resource list.
  'loading.dashboard': 'Loading the dashboard...',
  'loading.user': 'Loading user...',
  'loading.systemInfo': 'Loading system information...',
  'loading.asset': 'Loading asset...',
  'loading.job': 'Loading job...',
  // Dashboard.
  'dashboard.manageResources': 'Manage resources',
  'dashboard.manageSites': 'Manage sites',
  'dashboard.count': '{resource} ({count})',
  'dashboard.noSites': 'No sites.',
  'column.name': 'Name',
  'column.active': 'Active',
  'column.email': 'Email',
  'column.role': 'Role',
  'column.version': 'Version',
  'column.state': 'State',
  'column.author': 'Author',
  'column.description': 'Description',
  'column.filename': 'Filename',
  'column.altText': 'Alt text',
  'column.status': 'Status',
  'column.started': 'Started',
  'column.ended': 'Ended',
  // Dashboard and list resource names, the plural used in loading and paging.
  'resource.assets': 'assets',
  'resource.modules': 'modules',
  'resource.jobs': 'jobs',
  'resource.users': 'users',
  // Assets and jobs (`AssetEditForm`, `JobController`).
  'assets.legend': 'Asset',
  'assets.editLabel': 'Edit this asset',
  'assets.deleteLabel': 'Delete this asset',
  'assets.errorEmptyName': 'An asset must have a name.',
  'asset.file': 'File',
  'jobs.editLabel': 'Update this job',
  'jobs.deleteLabel': 'Delete this job',
  'jobs.update': 'Update',
  'jobs.rowName': 'Job #{id}',
  'jobs.args': 'Arguments',
  'jobs.log': 'Log',
  'jobs.logEmpty': 'The job has written no log yet.',
  'jobs.logFailed': 'The log could not be read: {message}',
  'jobs.refreshing': 'This page refreshes every 2 seconds while the job runs.',
  'jobs.class.importArchive': 'Archive import',
  'jobs.class.undoImport': 'Undo of an archive import',
  'jobs.openImport': 'Open the import',
  'jobs.noWritableMember':
    'A job is written by its dispatcher, so an update changes no member. The request finds the row and reads it back.',
  // Users.
  'users.legend': 'User information',
  'users.settingsLegend': 'User settings',
  'users.displayName': 'Display name',
  'users.email': 'Email',
  'users.role': 'Role',
  'users.selectRole': 'Select role...',
  'users.active': 'Active',
  'users.errorEmptyName': 'The name cannot be empty.',
  'users.errorNoRole': 'Users must have a role.',
  'users.add': 'Add user',
  'users.deleteLabel': 'Delete this user',
  'users.invite': 'Invite this user',
  'users.passwordLink': 'Create a password link',
  'users.linkLegend': 'Password link',
  'users.linkMailed': 'The invitation mail went to {email}.',
  'users.linkNotMailed': 'No mail was sent, so hand this link to the user.',
  'users.linkLabel': 'Password link for {name}',
  'users.copy': 'Copy link',
  'users.copied': 'Link copied.',
  'users.copyFailed': 'The browser refused clipboard access. Select the link and copy it by hand.',
  'users.editLink': 'Edit this user',
  'role.globalAdmin': 'Global Administrator',
  'role.siteAdmin': 'Supervisor',
  'role.editor': 'Editor',
  'role.reviewer': 'Reviewer',
  'role.author': 'Author',
  'role.researcher': 'Researcher',
  // Query and columns pages.
  'query.resourceType': 'Resource type',
  'query.search': 'Search',
  'query.idle': 'Enter a query and preview it.',
  'query.empty': 'No resources match the query.',
  'columns.resourceType': 'Resource type',
  'columns.legend': 'Browse columns',
  // Settings page.
  'settings.installationTitle': 'Installation title',
  'settings.administratorEmail': 'Administrator email',
  'settings.timeZone': 'Time zone',
  'settings.paginationPerPage': 'Pagination per page',
  'settings.defaultPrivateItemSets': 'Default to private item sets',
  'settings.indexFulltextSearch': 'Index fulltext search',
  'settings.versionNotifications': 'Version notifications',
  'settings.saved': 'Settings updated.',
  'settings.note': 'The port saves only the locale yet; the other changes are not persisted.',
  // `SettingForm` and `UserForm`; "Default" is `LocaleSelectFactory`'s empty option.
  'settings.locale': 'Locale',
  'settings.localeDefault': 'Default',
  // System information page.
  'systemInfo.omeka': 'Omeka S',
  'systemInfo.version': 'Version',
  'systemInfo.api': 'API',
  'systemInfo.baseUrl': 'Base URL',
  'systemInfo.resources': 'Resources',
  'systemInfo.tableLabel': 'System information',
  'route.invalidUserId': '"{param}" is not a valid user id.',
  'route.invalidAssetId': '"{param}" is not a valid asset id.',
  'route.invalidJobId': '"{param}" is not a valid job id.',
} as const;

/** A complete table of this namespace, in one locale. */
export type AdminMessages = Table<keyof typeof en>;

export const admin = defineNamespace('admin', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
