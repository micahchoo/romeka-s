/**
 * The two text lines a canvas card shows: the layout's catalogue label and a
 * one-line summary of the block's content.
 */
import type { Translator } from '../../../../i18n';
import type { BlockRegistry } from '../../blockSchema';
import type { BlockDraft } from '../draft';
import { dataObject } from '../values';

const MAX_SUMMARY = 80;

const asText = (value: unknown): string => (typeof value === 'string' ? value : '');

/** Tags and whitespace runs become single spaces, so a summary stays one line. */
export const plainText = (html: string): string =>
  html
    .replace(/<[^>]*>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

const clip = (text: string): string =>
  text.length > MAX_SUMMARY ? `${text.slice(0, MAX_SUMMARY - 1)}...` : text;

/** The message lookup a card reads its labels through. */
export type Translate = Translator['t'];

/** The card's content summary: HTML text, the page title, or an attachment count. */
export const blockSummary = (
  block: BlockDraft,
  pageTitle: string,
  t: Translate,
): string => {
  const data = dataObject(block);
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
    return t('canvas.attachments', { count: block.attachments.length });
  }
  return '';
};

/** The catalogue label for a layout, or the unknown marker for one not registered. */
export const layoutLabel = (registry: BlockRegistry, layout: string, t: Translate): string => {
  const entry = registry.layouts.find((candidate) => candidate.name === layout);
  return entry === undefined ? t('layout.unknown', { name: layout }) : t(entry.label);
};
