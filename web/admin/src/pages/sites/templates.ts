/**
 * The page templates: the built-in starting points and the per-site saved ones.
 *
 * A built-in template arrives from `src/generated/templates.json`, written by
 * `crates/omeka-site/src/blocks/templates.rs` from the registry beside the block
 * layouts. `templateRegistry.ts` parses that file, so this module carries only
 * the shapes and the saved-template half that the editor shell needs.
 *
 * A site's own templates live in ONE `site_setting` row, `romeka_page_templates`,
 * whose value is a JSON list of `{id, name, description, layout, layout_data,
 * blocks}`. Upstream ignores a `site_setting` key it does not read, so the row
 * is Romeka-only. The site document carries it under `o:site_settings`, and the
 * site write stores it back. [`savedTemplates`] reads that row at the boundary
 * and drops an entry that is not a template, so a corrupt row cannot break a
 * page that only wants to list them.
 */
import type { JsonValue } from '../../api/types';
import type { MessageKey } from '../../i18n';

/** The page layout a template starts from, the draft's own two values. */
export type TemplateLayout = 'normal' | 'grid';

/** A template block: the draft shape without a key or attachments. */
export interface TemplateBlock {
  /** The stored layout name, the carrier's for a Romeka block. */
  readonly layout: string;
  /** The block's `o:data`, an object or a list. */
  readonly data: JsonValue;
  /** The block's `o:layout_data`, or `null`. */
  readonly layoutData: JsonValue | null;
}

/** A template's content, shared by a built-in and a saved template. */
export interface TemplateContent {
  readonly id: string;
  readonly layout: TemplateLayout;
  readonly layoutData: JsonValue;
  readonly blocks: readonly TemplateBlock[];
}

/** A built-in template as generated: its name and description are catalogue keys. */
export interface BuiltinTemplate extends TemplateContent {
  readonly label: MessageKey;
  readonly description: MessageKey;
}

/** A saved template as the site setting row stores it. */
export interface SavedTemplate extends TemplateContent {
  readonly name: string;
  readonly description: string;
}

/** A template with a resolved name, as the chooser and the picker show it. */
export interface TemplateChoice extends TemplateContent {
  readonly name: string;
  readonly description: string;
}

/** The `site_setting` key a site's own templates live under. */
export const SAVED_TEMPLATES_KEY = 'romeka_page_templates';

const isJsonObject = (value: JsonValue): value is Readonly<Record<string, JsonValue>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const isJsonArray = (value: JsonValue): value is readonly JsonValue[] => Array.isArray(value);

const stringOf = (value: JsonValue | undefined): string =>
  typeof value === 'string' ? value : '';

/** One saved template's blocks, or `null` when the value is not a list of them. */
const savedBlocks = (value: JsonValue | undefined): readonly TemplateBlock[] | null => {
  if (value === undefined || !isJsonArray(value)) {
    return null;
  }
  const blocks: TemplateBlock[] = [];
  for (const entry of value) {
    if (!isJsonObject(entry)) {
      return null;
    }
    const data = entry.data;
    const layout = stringOf(entry.layout);
    if (layout === '' || data === undefined || (!isJsonObject(data) && !isJsonArray(data))) {
      return null;
    }
    const layoutData = entry.layout_data;
    blocks.push({
      layout,
      data,
      layoutData: layoutData === undefined ? null : layoutData,
    });
  }
  return blocks;
};

/** One saved template, or `null` when the value is not one. */
const savedTemplate = (value: JsonValue): SavedTemplate | null => {
  if (!isJsonObject(value)) {
    return null;
  }
  const id = stringOf(value.id);
  const name = stringOf(value.name);
  const layout = value.layout;
  const layoutData = value.layout_data;
  const blocks = savedBlocks(value.blocks);
  if (
    id === '' ||
    name === '' ||
    (layout !== 'normal' && layout !== 'grid') ||
    (layoutData !== undefined && !isJsonObject(layoutData) && !isJsonArray(layoutData)) ||
    blocks === null
  ) {
    return null;
  }
  return {
    id,
    name,
    description: stringOf(value.description),
    layout,
    layoutData: layoutData ?? {},
    blocks,
  };
};

/** The saved templates a site's settings carry, in stored order. */
export const savedTemplates = (siteSettings: JsonValue | null): readonly SavedTemplate[] => {
  if (siteSettings === null || !isJsonObject(siteSettings)) {
    return [];
  }
  const value = siteSettings[SAVED_TEMPLATES_KEY];
  if (value === undefined || !isJsonArray(value)) {
    return [];
  }
  const templates: SavedTemplate[] = [];
  for (const entry of value) {
    const template = savedTemplate(entry);
    if (template !== null) {
      templates.push(template);
    }
  }
  return templates;
};

/** One saved template as the site setting row stores it: `layout_data` and
 * `blocks[].layout_data`, the stored member names. */
const savedTemplateWire = (template: SavedTemplate): JsonValue => ({
  id: template.id,
  name: template.name,
  description: template.description,
  layout: template.layout,
  layout_data: template.layoutData,
  blocks: template.blocks.map((block) => ({
    layout: block.layout,
    data: block.data,
    layout_data: block.layoutData,
  })),
});

/** The site setting value for a saved-template list. */
export const savedTemplatesWire = (templates: readonly SavedTemplate[]): JsonValue =>
  templates.map(savedTemplateWire);

/** The site settings map with `templates` stored under the saved-templates key,
 * keeping every other row a site holds. */
export const withSavedTemplates = (
  siteSettings: JsonValue | null,
  templates: readonly SavedTemplate[],
): JsonValue => ({
  ...(siteSettings !== null && isJsonObject(siteSettings) ? siteSettings : {}),
  [SAVED_TEMPLATES_KEY]: savedTemplatesWire(templates),
});

let sequence = 0;

/** A new template id, unique within this session. */
export const newTemplateId = (): string => {
  sequence += 1;
  return `t${Date.now().toString(36)}-${sequence.toString(36)}`;
};

/** The built-in templates with their catalogue keys resolved to text. */
export const builtinChoices = (
  registry: readonly BuiltinTemplate[],
  translate: (key: MessageKey) => string,
): readonly TemplateChoice[] =>
  registry.map((template) => ({
    id: template.id,
    name: translate(template.label),
    description: translate(template.description),
    layout: template.layout,
    layoutData: template.layoutData,
    blocks: template.blocks,
  }));

/** The saved templates as the chooser and the picker show them. */
export const savedChoices = (templates: readonly SavedTemplate[]): readonly TemplateChoice[] =>
  templates.map((template) => ({
    id: template.id,
    name: template.name,
    description: template.description,
    layout: template.layout,
    layoutData: template.layoutData,
    blocks: template.blocks,
  }));
