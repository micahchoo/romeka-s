/**
 * The item type pages' messages, shared by the Item types and Fields routes.
 *
 * They live beside the pages rather than in `src/i18n/catalogue.ts` because the
 * English catalogue is part of the first paint (`perf/budget.json`, `PERF.md`).
 * English is here, in the page chunk; every other locale is its own chunk under
 * `locales/`, loaded through [`ITEM_TYPES_LOADERS`] as the Theme page loads its
 * table. Each table is typed against `en`, so a key added in English and
 * missing elsewhere is a `tsc` error, and a locale with no loader is one too.
 */
import { createContext, useContext, useEffect, useMemo, useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { useLocale, useTranslator } from "../../i18n";
import type { Locale, MessageParams } from "../../i18n";

export const en = {
  "action.change": "Change",
  "action.cancel": "Cancel",
  "list.lede":
    "An item type is the form you fill in for one kind of thing in your collection. A letter asks for a sender; a photograph asks who took it.",
  "list.new": "New item type",
  "list.resource": "item types",
  "list.empty": "No item types yet.",
  "list.type": "Item type",
  "list.fields": "Fields",
  "list.items": "Items",
  "list.class": "Standard class",
  "ask.question": "What kind of thing is this?",
  "ask.placeholder": "For example: Letter",
  "ask.hint":
    "Name one thing, not the whole collection. You make a separate type for each kind.",
  "ask.known": "Start a {name} type with these fields",
  "ask.unknown": "Start “{name}” as a physical object with these fields",
  "ask.useFields": "Use these fields",
  "ask.start": "Start",
  "ask.titleOnly": "Start with {field} only",
  "ask.after": "You can rename, reorder or remove any of them next.",
  "type.name": "Item type name",
  "type.namePlaceholder": "Name this item type",
  "type.create": "Create item type",
  "type.save": "Save item type",
  "type.delete": "Delete this item type",
  "type.errorName": "Give the item type a name.",
  "class.line": "Standard class: {name}",
  "class.none": "No standard class.",
  "class.noneShort": "None",
  "class.change": "Change the standard class",
  "class.pick": "Find a standard class",
  "class.pickHint": "Describe it, for example “photo”",
  "class.clear": "Use no standard class",
  "tabs.label": "Editor view",
  "tabs.fields": "Fields",
  "tabs.preview": "Preview",
  "fields.heading": "Fields, in the order they appear",
  "fields.empty": "This item type has no fields yet. Add one below.",
  "fields.add": "Add a field",
  "fields.addHint": "Describe it, for example “how many pages”",
  "field.name": "Name on the form for {term}",
  "field.moveHandle":
    "Move {name}, field {position} of {count}. Use the up and down arrow keys, or drag.",
  "field.moved": "{name} is now field {position} of {count}.",
  "field.moveUp": "Move {name} up",
  "field.moveDown": "Move {name} down",
  "field.remove": "Remove {name}",
  "field.changeTerm": "Change the standard term of {name}",
  "field.pickTerm": "Find a standard term",
  "field.pickHint": "Describe it, for example “who wrote it”",
  "field.required": "{name} is required",
  "field.requiredShort": "Required",
  "field.private": "{name} is private",
  "field.privateShort": "Private",
  "field.more": "Help text and language",
  "field.help": "Help text on the form",
  "field.language": "Language of new values",
  "accepts.label": "Accepts:",
  "accepts.any": "Text, a web address or another record",
  "accepts.change": "Change what {name} accepts",
  "accepts.legend": "What {name} accepts",
  "accepts.note":
    "Tick several to let people choose. None ticked accepts text, a web address or another record.",
  "accepts.literal": "Text",
  "accepts.uri": "Web address",
  "accepts.resource": "Any record",
  "accepts.item": "Another item",
  "accepts.itemSet": "An item set",
  "accepts.media": "A media file",
  "accepts.missing": "{id} (not installed)",
  "use.legend": "Used for",
  "use.items": "Items",
  "use.itemSets": "Item sets",
  "use.media": "Media",
  "use.note":
    "Only narrows the Type list on those forms, in this browser. The type stays usable everywhere.",
  "advanced.summary": "Advanced",
  "advanced.title": "Title comes from",
  "advanced.titleDefault": "Title (the default)",
  "advanced.description": "Description comes from",
  "advanced.descriptionDefault": "Description (the default)",
  "preview.unnamed": "item",
  "preview.label": "Preview of the form for a new {name}",
  "preview.heading": "What people see when adding a {name}",
  "preview.title": "New {name}",
  "preview.note": "A preview. Nothing typed here is saved.",
  "fieldsPage.lede":
    "Every field you can add to an item type, in plain words. Each one is saved as a standard term, so other archives can read your records.",
  "fieldsPage.search": "Find a field",
  "fieldsPage.searchHint": "Describe it, for example “who wrote it”",
  "fieldsPage.count": "{count} fields",
  "fieldsPage.matches": "{count} fields match",
  "fieldsPage.called": "· called {names}",
  "fieldsPage.calledIn": "“{label}” in {type}",
  "fieldsPage.used": "Item types using it: {count}",
  "fieldsPage.unused": "Not used yet",
} as const;

/** An item type page message id. */
export type ItemTypesMessageKey = keyof typeof en;

/** A complete item type pages table: every message id, no fallback at read time. */
export type ItemTypesMessages = Readonly<Record<ItemTypesMessageKey, string>>;

/** How each locale's table arrives. `Record<Locale, …>` makes a missing locale a `tsc` error. */
const ITEM_TYPES_LOADERS: Readonly<
  Record<Locale, () => Promise<ItemTypesMessages>>
> = {
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

const loaded = new Map<Locale, ItemTypesMessages>([["en", en]]);

/**
 * The active locale's table, or `null` until the first one arrives. A later
 * locale switch keeps the previous table until the new one loads, as the main
 * store keeps its translator, so an open draft never unmounts to wait.
 */
const useItemTypesMessages = (): ItemTypesMessages | null => {
  const locale = useLocale();
  const [table, setTable] = useState<ItemTypesMessages | null>(
    () => loaded.get(locale) ?? null,
  );
  useEffect(() => {
    const cached = loaded.get(locale);
    if (cached !== undefined) {
      setTable(cached);
      return undefined;
    }
    let current = true;
    void ITEM_TYPES_LOADERS[locale]().then((messages) => {
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

/** The table the pages render with. English outside a provider, as in a component test. */
const ItemTypesMessagesContext = createContext<ItemTypesMessages>(en);

/**
 * Renders its children once the active locale's table has arrived, so no label
 * renders in English and then swaps.
 */
export const WithItemTypesMessages = ({
  children,
}: {
  readonly children: ReactNode;
}): ReactElement => {
  const { t } = useTranslator();
  const messages = useItemTypesMessages();
  if (messages === null) {
    return <p role="status">{t("loading.form")}</p>;
  }
  return (
    <ItemTypesMessagesContext.Provider value={messages}>
      {children}
    </ItemTypesMessagesContext.Provider>
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

/** A lookup into the item type pages' table. */
export type ItemTypesText = (
  key: ItemTypesMessageKey,
  params?: MessageParams,
) => string;

/** The item type pages' message lookup in the table [`WithItemTypesMessages`] provides. */
export const useItemTypesText = (): ItemTypesText => {
  const messages = useContext(ItemTypesMessagesContext);
  return useMemo(
    () =>
      (key: ItemTypesMessageKey, params?: MessageParams): string =>
        interpolate(messages[key], params),
    [messages],
  );
};
