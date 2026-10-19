/**
 * The Standards and Fields page messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `StandardsMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'standards.lede':
    'The shared vocabularies your fields and item types are saved in. You do not need to change them to catalogue a collection.',
  'standards.define': 'What {term} means',
  'standards.import': 'Import a vocabulary',
  "import.heading": "Import a vocabulary",
  "import.lede":
    "Bring in a published vocabulary from an RDF file or a web address. Its classes and properties become available to your fields and item types.",
  "import.submit": "Import vocabulary",
  "import.working": "Reading the vocabulary…",
  "source.legend": "Where the vocabulary comes from",
  "source.upload": "A file on this computer",
  "source.url": "A web address",
  "source.file": "Vocabulary file",
  "source.urlLabel": "Web address of the file",
  "source.format": "File format",
  "source.formatHelp":
    "Leave this on “Detect automatically” unless the import says it cannot tell the format.",
  "format.guess": "Detect automatically",
  "format.rdfxml": "RDF/XML (.rdf, .owl)",
  "format.turtle": "Turtle or Notation3 (.ttl, .n3)",
  "format.ntriples": "N-Triples (.nt)",
  "format.jsonld": "JSON-LD (.jsonld)",
  "about.legend": "About the vocabulary",
  "about.namespace": "Namespace URI",
  "about.namespaceHelp":
    "The web address every class and property of the vocabulary starts with, such as http://xmlns.com/foaf/0.1/. Only the terms under it are imported. It usually ends with / or #.",
  "about.prefix": "Prefix",
  "about.prefixHelp":
    "A short name written before each term, as in foaf:name. Use letters, numbers, hyphens and underscores.",
  "about.label": "Label",
  "about.labelHelp": "The name people see for this vocabulary.",
  "about.comment": "Description (optional)",
  "vocabularies.advancedSummary": "Advanced",
  "advanced.lang": "Preferred language",
  "advanced.langHelp":
    "A language tag such as en or fr. Labels and comments in that language are kept. Without one, or for a term that has none in it, the first in the file is kept.",
  "advanced.label": "Label property",
  "advanced.labelHelp":
    "Only for a vocabulary that names its terms with an unusual property. Write the full property URI in angle brackets, such as <http://schema.org/name>. Without one, skos:prefLabel, rdfs:label, foaf:name, rss:title and dc:title are tried in turn.",
  "advanced.comment": "Comment property",
  "advanced.commentHelp":
    "The same, for the description of each term. Without one, rdfs:comment is read.",
  "error.noFile": "Choose a file to import.",
  "error.noUrl": "Enter the web address of the file.",
  "update.toggle": "Update from a file or web address",
  "update.lede":
    "Compare this vocabulary with a newer copy of its file. You see every change before anything is saved.",
  "update.compare": "Compare",
  "update.comparing": "Comparing…",
  "diff.review": "Review these changes before you apply them.",
  "diff.none":
    "This vocabulary already matches the file. There is nothing to change.",
  "diff.newClasses": "New classes ({count})",
  "diff.newProperties": "New properties ({count})",
  "diff.changed": "Changed labels and comments ({count})",
  "diff.removed": "No longer in the file ({count})",
  "diff.removedNote":
    "These stay in the archive, and so do the values and item types that use them. Delete one from its own page if you no longer need it.",
  "diff.term": "Term",
  "diff.label": "Label",
  "diff.comment": "Comment",
  "diff.what": "What changes",
  "diff.now": "Now",
  "diff.file": "In the file",
  "diff.blank": "(none)",
  "diff.apply": "Apply these changes",
  "diff.applying": "Applying…",
  "diff.cancel": "Cancel",
  "diff.applied": "The changes are saved.",
  'page.properties': 'Properties',
  'loading.vocabulary': 'Loading vocabulary...',
  'loading.property': 'Loading property...',
  'loading.resourceClass': 'Loading resource class...',
  'loading.vocabularyForm': 'Loading vocabulary form...',
  'loading.propertyForm': 'Loading property form...',
  'loading.resourceClassForm': 'Loading resource class form...',
  'column.comment': 'Comment',
  'column.term': 'Term',
  'column.prefix': 'Prefix',
  'column.namespace': 'Namespace',
  'column.vocabulary': 'Vocabulary',
  'column.properties': 'Properties',
  'resource.properties': 'properties',
  'resource.resourceClasses': 'resource classes',
  'resource.vocabularies': 'vocabularies',
  // The vocabulary family detail pages (`VocabularyController::showDetailsAction`,
  // `PropertyController::showDetailsAction`, `ResourceClassController::showDetailsAction`,
  // `ResourceTemplateController::showAction`). A label whose upstream string is in
  // the catalogue keeps that string; the rest name a wire member.
  'detail.namespaceUri': 'Namespace URI',
  'detail.localName': 'Local name',
  'detail.classes': 'Classes',
  'detail.noProperties': 'This vocabulary has no properties.',
  'detail.noClasses': 'This vocabulary has no classes.',
  // The vocabulary family write surface (`VocabularyForm`, `VocabularyForm`
  // member lists, the property and class adapters, `ResourceTemplateForm`).
  'vocabularies.add': 'Add vocabulary',
  'vocabularies.legend': 'Vocabulary',
  'vocabularies.editLabel': 'Edit this vocabulary',
  'vocabularies.deleteLabel': 'Delete this vocabulary',
  'vocabularies.permanent':
    'Dublin Core and Dublin Core Type are part of the software, so this vocabulary cannot be edited or deleted.',
  'vocabularies.errorEmptyNamespace': 'The namespace URI cannot be empty.',
  'vocabularies.errorEmptyPrefix': 'The prefix cannot be empty.',
  'vocabularies.errorPrefixPattern':
    'The prefix may only contain letters, numbers, hyphens, and underscores.',
  'member.legend': 'Member',
  'member.selectVocabulary': 'Select a vocabulary',
  'member.errorEmptyLocalName': 'The local name cannot be empty.',
  'member.errorEmptyVocabulary': 'A vocabulary must be set.',
  'properties.add': 'Add property',
  'properties.editLabel': 'Edit this property',
  'properties.deleteLabel': 'Delete this property',
  'resourceClasses.add': 'Add resource class',
  'resourceClasses.editLabel': 'Edit this resource class',
  'resourceClasses.deleteLabel': 'Delete this resource class',
  'route.invalidVocabularyId': '"{param}" is not a valid vocabulary id.',
  'route.invalidPropertyId': '"{param}" is not a valid property id.',
  'route.invalidClassId': '"{param}" is not a valid resource class id.',
} as const;

/** A complete table of this namespace, in one locale. */
export type StandardsMessages = Table<keyof typeof en>;

export const standards = defineNamespace('standards', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
