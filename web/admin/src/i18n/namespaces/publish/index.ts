/**
 * The Publish panel messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `PublishMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'publish.heading': 'Publish',
  'publish.ledeTermList':
    'Publishing gives this list and each of its terms a permanent web address that people and programs can read.',
  'publish.ledeVocabulary':
    'Publishing gives this standard and each of its terms a permanent web address that people and programs can read.',
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

/** A complete table of this namespace, in one locale. */
export type PublishMessages = Table<keyof typeof en>;

export const publish = defineNamespace('publish', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
