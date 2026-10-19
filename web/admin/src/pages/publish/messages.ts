/**
 * The Publish panel's messages, on the term list page and the vocabulary page.
 *
 * They live beside the panel rather than in `src/i18n/catalogue.ts`, because
 * the main catalogue loads with the first paint and its budget is tight
 * (`perf/budget.json`). English is here; every other locale is its own chunk
 * under `locales/`. Each table is typed against `en`, so a key added in
 * English and missing elsewhere is a `tsc` error.
 */
import { use, useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale, MessageParams } from '../../i18n';

export const en = {
  'publish.heading': 'Publish',
  'publish.ledeTermList':
    'Publishing gives this list and each of its terms a permanent web address that people and programs can read.',
  'publish.ledeVocabulary':
    'Publishing gives this vocabulary and each of its terms a permanent web address that people and programs can read.',
  'publish.loading': 'Loading the publication...',
  'publish.stateDraft': 'Draft. Not published yet.',
  'publish.statePublished': 'Published. Version {version}, {date}.',
  'publish.publicUrl': 'Public address',
  'publish.promise':
    'Once published, terms can be added or deprecated, but never removed or renamed, and the address never changes. Other archives may already link to them.',
  'publish.slugLabel': 'Address name',
  'publish.slugHint': 'Lowercase letters, digits and single hyphens. It is served at {url}',
  'publish.slugInvalid': 'Use lowercase letters, digits and single hyphens, up to 64 characters.',
  'publish.namespaceLegend': 'Addresses of the terms',
  'publish.namespaceArchive': "This archive's address: {url}",
  'publish.namespacePersistent': 'A persistent address at w3id.org or purl.org',
  'publish.namespaceWhy':
    'A persistent address outlives a move to another domain: the service redirects it here, so links to your terms keep working if this archive moves.',
  'publish.namespaceLabel': 'Persistent namespace',
  'publish.namespaceHint': 'For example https://w3id.org/your-archive/places/. End it with / or #.',
  'publish.namespaceInvalid': 'Use an address at w3id.org or purl.org that ends with / or #.',
  'publish.redirectHelp': 'Then ask the service to redirect it here, for example with this w3id.org rule:',
  'publish.namespace': 'Namespace',
  'publish.termsHeading': 'Terms',
  'publish.columnTerm': 'Term',
  'publish.columnUri': 'URI',
  'publish.columnState': 'State',
  'publish.columnChange': 'Change',
  'publish.termDraft': 'Not published yet',
  'publish.termPublished': 'Published',
  'publish.termDeprecated': 'Deprecated',
  'publish.termDeprecationPending': 'Deprecated from the next version',
  'publish.deprecate': 'Deprecate',
  'publish.deprecateLabel': 'Deprecate {term}',
  'publish.undeprecate': 'Keep',
  'publish.undeprecateLabel': 'Keep {term} in use',
  'publish.links': 'Links',
  'publish.linksLabel': 'Links of {term} to an authority',
  'publish.exactMatch': 'The same term elsewhere (exactMatch)',
  'publish.closeMatch': 'A near term elsewhere (closeMatch)',
  'publish.linksHint': 'The address of this term in an authority such as GeoNames or Wikidata.',
  'publish.saveLinks': 'Save links',
  'publish.matchInvalid': 'Use an address that starts with http:// or https://.',
  'publish.publish': 'Publish',
  'publish.publishVersion': 'Publish version {version}',
  'publish.noChanges': 'Nothing changed since version {version}.',
  'publish.pending': 'Changes since version {version} are not public yet. Publish to add them.',
  'publish.published': 'Published version {version}.',
  'publish.versionsHeading': 'Versions',
  'publish.versionLine': 'Version {version}, {date}, {terms} terms',
  'publish.previewHeading': 'Preview in Turtle',
  'publish.replacedBy': 'Replaced by “{term}”',
} as const;

/** A Publish panel message id. */
export type PublishMessageKey = keyof typeof en;

/** A complete Publish panel table. */
export type PublishMessages = Readonly<Record<PublishMessageKey, string>>;

const LOADERS: Readonly<Record<Locale, () => Promise<PublishMessages>>> = {
  en: () => Promise.resolve(en),
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
};

const loads = new Map<Locale, Promise<PublishMessages>>([['en', Promise.resolve(en)]]);
const loaded = new Map<Locale, PublishMessages>([['en', en]]);

// A table not loaded yet suspends the caller, so the page's Suspense boundary
// waits for it instead of every caller handling a missing table.
const tableFor = (locale: Locale): PublishMessages | Promise<PublishMessages> => {
  const ready = loaded.get(locale);
  if (ready !== undefined) {
    return ready;
  }
  let load = loads.get(locale);
  if (load === undefined) {
    load = LOADERS[locale]().then((messages) => {
      loaded.set(locale, messages);
      return messages;
    });
    loads.set(locale, load);
  }
  return load;
};

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The Publish panel's message lookup in the current locale. */
export const usePublishText = (): ((key: PublishMessageKey, params?: MessageParams) => string) => {
  const locale = useLocale();
  const table = tableFor(locale);
  const messages = table instanceof Promise ? use(table) : table;
  return useMemo(
    () =>
      (key: PublishMessageKey, params?: MessageParams): string =>
        interpolate(messages[key], params),
    [messages],
  );
};
