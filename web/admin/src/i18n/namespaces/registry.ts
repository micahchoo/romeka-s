/**
 * Every message namespace, for the tools: `scripts/translate-locales.mjs`
 * fills each one's locale files after the shell catalogue, in this order, and
 * `scripts/split-catalogue.mjs` writes into them. The app never imports this
 * file; a route imports the namespace it reads from `<id>/index.ts`.
 *
 * `tests/i18n-sync.test.ts` checks that every directory here is listed and
 * that each one's `defineNamespace` id is its directory name.
 */
export interface NamespaceRecord {
  /** The directory under `namespaces/` and the id `defineNamespace` takes. */
  readonly id: string;
  /** How a locale file names the table: "The Hindi <what>." */
  readonly what: string;
  /** The type every locale file of the namespace is declared with. */
  readonly type: string;
}

export const NAMESPACES: readonly NamespaceRecord[] = [
  { id: 'common', what: 'shared page messages', type: 'CommonMessages' },
  { id: 'resources', what: 'item, item set and media page messages', type: 'ResourcesMessages' },
  { id: 'sites', what: 'site page messages', type: 'SitesMessages' },
  { id: 'admin', what: 'admin page messages', type: 'AdminMessages' },
  { id: 'account', what: 'account page messages', type: 'AccountMessages' },
  { id: 'standards', what: 'Standards and Fields page messages', type: 'StandardsMessages' },
  { id: 'editor', what: 'page editor messages', type: 'EditorMessages' },
  { id: 'content-blocks', what: 'content block messages', type: 'ContentBlockMessages' },
  { id: 'theme', what: 'Theme page messages', type: 'ThemeMessages' },
  { id: 'term-lists', what: 'Term lists messages', type: 'TermListMessages' },
  { id: 'item-types', what: 'item type pages messages', type: 'ItemTypesMessages' },
  { id: 'lessons', what: 'lessons messages', type: 'LessonsMessages' },
  { id: 'publish', what: 'Publish panel messages', type: 'PublishMessages' },
  { id: 'import', what: 'Import page messages', type: 'ImportMessages' },
  { id: 'import-lists', what: 'Import review list messages', type: 'ImportListsMessages' },
  { id: 'import-simple', what: 'Import page Simple screen messages', type: 'ImportSimpleMessages' },
  { id: 'import-structure', what: 'Import Structure screen messages', type: 'ImportStructureMessages' },
  { id: 'import-fixes', what: 'words of the Structure screen\'s corrections', type: 'ImportFixesMessages' },
  { id: 'import-advanced', what: 'Import Advanced screen messages', type: 'ImportAdvancedMessages' },
  { id: 'import-run', what: 'import run screen messages', type: 'ImportRunMessages' },
  { id: 'annotations', what: 'media notes panel messages', type: 'AnnotationsMessages' },
  { id: 'media-add', what: 'add-media panel messages', type: 'MediaAddMessages' },
  { id: 'bulk', what: 'bulk change panel messages', type: 'BulkMessages' },
  { id: 'bulk-delete', what: 'bulk delete messages', type: 'BulkDeleteMessages' },
];
