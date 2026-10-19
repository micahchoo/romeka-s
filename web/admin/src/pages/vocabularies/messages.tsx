/**
 * The vocabulary import and update messages.
 *
 * They live beside the pages rather than in `src/i18n/catalogue.ts` because the
 * English catalogue is part of the first paint (`perf/budget.json`, `PERF.md`).
 * English is here, in the page chunk; every other locale is its own chunk under
 * `locales/`, loaded through [`IMPORT_LOADERS`]. Each table is typed against
 * `en`, so a key added in English and missing elsewhere is a `tsc` error.
 */
import { createContext, useContext, useEffect, useMemo, useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { useLocale, useTranslator } from "../../i18n";
import type { Locale, MessageParams } from "../../i18n";

export const en = {
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
  "advanced.summary": "Advanced",
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
} as const;

/** A vocabulary import message id. */
export type ImportMessageKey = keyof typeof en;

/** A complete import messages table: every message id, no fallback at read time. */
export type ImportMessages = Readonly<Record<ImportMessageKey, string>>;

/** How each locale's table arrives. `Record<Locale, …>` makes a missing locale a `tsc` error. */
const IMPORT_LOADERS: Readonly<Record<Locale, () => Promise<ImportMessages>>> =
  {
    en: () => Promise.resolve(en),
    es: () => import("./locales/es").then((module) => module.es),
    hi: () => import("./locales/hi").then((module) => module.hi),
    mr: () => import("./locales/mr").then((module) => module.mr),
    ml: () => import("./locales/ml").then((module) => module.ml),
    ta: () => import("./locales/ta").then((module) => module.ta),
    kn: () => import("./locales/kn").then((module) => module.kn),
    te: () => import("./locales/te").then((module) => module.te),
    ne: () => import("./locales/ne").then((module) => module.ne),
  };

const loaded = new Map<Locale, ImportMessages>([["en", en]]);

/** The active locale's table, or `null` until it first arrives. */
const useImportMessages = (): ImportMessages | null => {
  const locale = useLocale();
  const [table, setTable] = useState<ImportMessages | null>(
    () => loaded.get(locale) ?? null,
  );
  useEffect(() => {
    const cached = loaded.get(locale);
    if (cached !== undefined) {
      setTable(cached);
      return undefined;
    }
    let current = true;
    void IMPORT_LOADERS[locale]().then((messages) => {
      loaded.set(locale, messages);
      if (current) {
        setTable(messages);
      }
    });
    return () => {
      current = false;
    };
  }, [locale]);
  return table;
};

const ImportMessagesContext = createContext<ImportMessages>(en);

/** Renders its children once the active locale's table has arrived. */
export const WithImportMessages = ({
  children,
}: {
  readonly children: ReactNode;
}): ReactElement => {
  const { t } = useTranslator();
  const messages = useImportMessages();
  if (messages === null) {
    return <p role="status">{t("loading.form")}</p>;
  }
  return (
    <ImportMessagesContext.Provider value={messages}>
      {children}
    </ImportMessagesContext.Provider>
  );
};

const interpolate = (
  template: string,
  params: MessageParams | undefined,
): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** A lookup into the import messages table. */
export type ImportText = (
  key: ImportMessageKey,
  params?: MessageParams,
) => string;

/** The import message lookup in the table [`WithImportMessages`] provides. */
export const useImportText = (): ImportText => {
  const messages = useContext(ImportMessagesContext);
  return useMemo(
    () =>
      (key: ImportMessageKey, params?: MessageParams): string =>
        interpolate(messages[key], params),
    [messages],
  );
};
