/**
 * The two text lines a canvas card shows: the layout's catalogue label and a
 * one-line summary of the block's content.
 */
import type { KeyOf, MessageKey, Translator } from '../../../../i18n';
import type { sites } from '../../../../i18n/namespaces/sites';
import type { BlockRegistry } from '../../blockSchema';
import type { TemplateBlock } from '../../templates';
import type { BlockDraft, LayoutData } from '../draft';
import { counted } from '../messages';
import type { EditorMessageKey } from '../messages';
import { carrierOf, romkaFor } from '../romeka';
import { SECTION_LAYOUT_PATH, sectionOf } from '../sections';
import { dataObject, readLayoutData } from '../values';

const MAX_SUMMARY = 80;

const asText = (value: unknown): string => (typeof value === 'string' ? value : '');

/** Tags and whitespace runs become single spaces, so a summary stays one line,
 * and entities read as the characters they stand for (`&quot;` as `"`). */
const plainText = (html: string): string =>
  new DOMParser()
    .parseFromString(html.replace(/<[^>]*>/g, ' '), 'text/html')
    .documentElement.textContent.replace(/\s+/g, ' ')
    .trim();

const clip = (text: string): string =>
  text.length > MAX_SUMMARY ? `${text.slice(0, MAX_SUMMARY - 1)}...` : text;

/** The message lookup a card reads its labels through. */
export type Translate = Translator<MessageKey | KeyOf<typeof sites> | EditorMessageKey>['t'];

/** The card's content summary: HTML text, the page title, or an attachment count. */
export const blockSummary = (
  block: BlockDraft,
  pageTitle: string,
  t: Translate,
): string => {
  const data = dataObject(block);
  const carrier = carrierOf(block);
  if (carrier !== null) {
    const text = clip(asText(carrier.settings.heading).trim());
    if (text !== '') {
      return text;
    }
  }
  if (block.layout === 'html') {
    const text = clip(plainText(asText(data.html)));
    if (text !== '') {
      return text;
    }
  }
  if (block.layout === 'pageTitle') {
    const title = clip(pageTitle.trim());
    if (title !== '') {
      return title;
    }
  }
  if (block.attachments.length > 0) {
    return counted(t, 'attachments', block.attachments.length);
  }
  return '';
};

/** The catalogue label for a block: its Romeka block when it carries one, else
 * its layout, else the unknown marker. */
export const layoutLabel = (
  registry: BlockRegistry,
  block: { readonly layout: string; readonly layoutData: LayoutData | null },
  t: Translate,
): string => {
  const romka = romkaFor(registry, block);
  if (romka !== undefined) {
    return t(romka.label);
  }
  const section = sectionOf(block);
  if (section !== null) {
    return t('section.named', { layout: sectionLabel(registry, section.layout, t) });
  }
  const entry = registry.layouts.find((candidate) => candidate.name === block.layout);
  return entry === undefined ? t('layout.unknown', { name: block.layout }) : t(entry.label);
};

/** The name of a section layout, from the section schema's own option label. */
export const sectionLabel = (registry: BlockRegistry, layout: string, t: Translate): string => {
  const field = registry.sectionLayoutData.fields.find((entry) => entry.key === SECTION_LAYOUT_PATH);
  const option =
    field?.type === 'choice' ? field.options.find((entry) => entry.value === layout) : undefined;
  return option === undefined ? layout : t(option.label);
};

/** The labels of a template's blocks, one per block, for its chooser card. */
export const templateOutline = (
  registry: BlockRegistry,
  blocks: readonly TemplateBlock[],
  t: Translate,
): readonly string[] =>
  blocks.map((block) =>
    layoutLabel(
      registry,
      { layout: block.layout, layoutData: readLayoutData(block.layoutData) },
      t,
    ),
  );
