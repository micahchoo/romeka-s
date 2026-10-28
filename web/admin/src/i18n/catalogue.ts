/**
 * The shell catalogue in English: the source language, keyed by message id.
 *
 * It holds only what the shell renders before any route loads: the title, the
 * nav, the sign-in panel, the locale switch, the route boundary and two
 * formats. It is in the first paint, so every other string lives in a
 * namespace under `namespaces/` (`namespace.ts`). `en` is the shape every
 * other shell catalogue is checked against, so adding a key to `en` makes a
 * catalogue that lacks it a `tsc` error. A string frozen from upstream carries
 * the upstream message id as its key where one exists.
 *
 * Placeholders are `{name}` and are filled by `Translator::t`.
 */
export const en = {
  // Application shell and navigation.
  'app.title': 'Romeka S admin',
  'shell.navLabel': 'Admin pages',
  'shell.noPage': 'No such page. Go to the admin dashboard.',
  'shell.loadFailed': 'This page could not load. Check the connection, then reload.',
  'shell.reload': 'Reload',
  'group.content': 'Content',
  'group.admin': 'Admin',
  'page.dashboard': 'Admin dashboard',
  'page.items': 'Items',
  'page.itemSets': 'Item sets',
  'page.media': 'Media',
  'page.import': 'Import',
  'page.sites': 'Sites',
  'page.fields': 'Fields',
  'page.users': 'Users',
  'page.modules': 'Modules',
  'page.jobs': 'Jobs',
  'page.settings': 'Settings',
  'page.assets': 'Assets',
  'page.systemInfo': 'System information',
  'page.columns': 'Browse columns',
  'page.query': 'Search query',
  'loading.page': 'Loading page...',
  'auth.panelHeading': 'Sign in',
  'auth.email': 'Email',
  'auth.password': 'Password',
  'auth.errorEmpty': 'Enter your email and password.',
  'auth.keyDisclosure': 'Use an API key',
  'auth.forgot': 'Forgot your password?',
  'auth.heading': 'Sign in with an API key',
  'auth.identity': 'Key identity',
  'auth.credential': 'Key credential',
  'auth.submit': 'Sign in',
  'auth.hint': 'An installation administrator mints a key with',
  'auth.signedInAs': 'Signed in as {identity}',
  'auth.signedOut': 'Signed out',
  'auth.signIn': 'Sign in',
  'auth.signOut': 'Sign out',
  // Errors and formatting (`src/app/format.ts`).
  'format.httpSuffix': ' (HTTP {status})',
  'format.bytes': '{size} bytes',
  'field.saving': 'Saving...',
  // The locale switch.
  'locale.label': 'Language',
  'locale.en': 'English',
  'locale.es': 'Español',
  'locale.hi': 'हिन्दी',
  'locale.mr': 'मराठी',
  'locale.ml': 'മലയാളം',
  'locale.ta': 'தமிழ்',
  'locale.kn': 'ಕನ್ನಡ',
  'locale.te': 'తెలుగు',
  'locale.ne': 'नेपाली',
  // Term lists (the Custom Vocab module built into core) and the term picker.
  'page.termLists': 'Term lists',
  // The Describe nav group and its Advanced > Standards link.
  'group.describe': 'Describe',
  'nav.advanced': 'Advanced',
  'page.itemTypes': 'Item types',
  'page.standards': 'Standards',
  // The jump-to box in the header and the saved lists under Items.
  'jump.label': 'Jump to a record',
  'jump.placeholder': '#id, identifier or title',
  'nav.savedLists': 'Saved lists',
} as const;

/** Every message id in the catalogue. A key the English catalogue names. */
export type MessageKey = keyof typeof en;

/** A complete catalogue: every message id, no fallback at read time. */
export type Catalogue = Readonly<Record<MessageKey, string>>;

/** Values the translator substitutes into a message's `{name}` placeholders. */
export type MessageParams = Readonly<Record<string, string | number>>;

/** Every message id, in catalogue order. */
export const MESSAGE_KEYS: readonly string[] = Object.keys(en);

/**
 * The message ids a candidate catalogue is missing. The shipped catalogues are
 * `Catalogue` typed, so this returns an empty list for them; the guard exists
 * so a catalogue loaded at runtime, and the test that proves a missing key
 * fails, share one implementation.
 */
export const missingKeys = (catalogue: Readonly<Record<string, string>>): readonly string[] =>
  MESSAGE_KEYS.filter((key) => !(key in catalogue));
