/**
 * The page block editor: add, remove, reorder and edit the blocks of one site
 * page. A block's `o:data` and `o:layout_data` are edited as JSON text, and the
 * text is parsed at submit, so bad JSON becomes a form error, never a crash.
 */
import type { ReactElement } from 'react';
import { parseJsonObjectOrArrayText, parseJsonText } from '../../api/parse';
import type { JsonValue, SiteBlockWrite, SitePageBlock } from '../../api/types';
import { formatJson } from '../../app/json';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { Translator } from '../../i18n';
import { blockLayoutOptions } from './layouts';

/** One editable block: the layout plus its two JSON members as text. */
export interface BlockDraft {
  readonly key: number;
  readonly layout: string;
  readonly data: string;
  /** Empty means the block has no layout data, so the member is not sent. */
  readonly layoutData: string;
}

export const blocksFromPage = (blocks: readonly SitePageBlock[]): readonly BlockDraft[] =>
  blocks.map((block, index) => ({
    key: index,
    layout: block.layout,
    data: formatJson(block.data),
    layoutData: block.layoutData === null ? '' : formatJson(block.layoutData),
  }));

const nextKey = (blocks: readonly BlockDraft[]): number =>
  blocks.reduce((max, block) => Math.max(max, block.key), -1) + 1;

/** A new block: an empty HTML payload, the layout most edits start from. */
export const addBlock = (blocks: readonly BlockDraft[]): readonly BlockDraft[] => [
  ...blocks,
  { key: nextKey(blocks), layout: 'html', data: '{}', layoutData: '' },
];

export const updateBlock = (
  blocks: readonly BlockDraft[],
  key: number,
  patch: Partial<Omit<BlockDraft, 'key'>>,
): readonly BlockDraft[] =>
  blocks.map((block) => (block.key === key ? { ...block, ...patch } : block));

export const removeBlock = (blocks: readonly BlockDraft[], key: number): readonly BlockDraft[] =>
  blocks.filter((block) => block.key !== key);

/** Moves a block one position; a move past either end is a no-op. */
export const moveBlock = (
  blocks: readonly BlockDraft[],
  key: number,
  delta: -1 | 1,
): readonly BlockDraft[] => {
  const from = blocks.findIndex((block) => block.key === key);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= blocks.length) {
    return blocks;
  }
  const moved = blocks[from];
  if (moved === undefined) {
    return blocks;
  }
  const next = blocks.filter((block) => block.key !== key);
  next.splice(to, 0, moved);
  return next;
};

/** A parse failure, split so the message is built with the catalogue. */
export type BlockWriteError =
  | { readonly kind: 'data'; readonly position: number; readonly message: string }
  | { readonly kind: 'layoutData'; readonly position: number; readonly message: string };

export type BlockWrites =
  | { readonly blocks: readonly SiteBlockWrite[]; readonly error: null }
  | { readonly blocks: null; readonly error: BlockWriteError };

/** The catalogue message for a parse failure. */
export const blockWriteError = (error: BlockWriteError, t: Translator): string =>
  error.kind === 'data'
    ? t.t('blocks.errorData', { position: error.position, message: error.message })
    : t.t('blocks.errorLayoutData', { position: error.position, message: error.message });

/** Parses every block's JSON, stopping at the first error with its position. */
export const toBlockWrites = (drafts: readonly BlockDraft[]): BlockWrites => {
  const blocks: SiteBlockWrite[] = [];
  for (const [index, draft] of drafts.entries()) {
    const position = index + 1;
    let data: JsonValue;
    try {
      data = parseJsonObjectOrArrayText(draft.data);
    } catch (error) {
      return { blocks: null, error: { kind: 'data', position, message: toError(error).message } };
    }
    let layoutData: JsonValue | null = null;
    if (draft.layoutData.trim() !== '') {
      try {
        layoutData = parseJsonText(draft.layoutData);
      } catch (error) {
        return {
          blocks: null,
          error: { kind: 'layoutData', position, message: toError(error).message },
        };
      }
    }
    blocks.push({ layout: draft.layout, data, layoutData });
  }
  return { blocks, error: null };
};

export interface BlockEditorProps {
  readonly blocks: readonly BlockDraft[];
  readonly onChange: (blocks: readonly BlockDraft[]) => void;
}

export const BlockEditor = ({ blocks, onChange }: BlockEditorProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <fieldset>
      <legend>{t('blocks.legend')}</legend>
      <ul className="block-rows">
        {blocks.map((block, index) => {
          const position = index + 1;
          return (
            <li key={block.key} className="block-row">
              <p>
                <label htmlFor={`block-${String(block.key)}-layout`}>
                  {t('blocks.layout', { position })}
                </label>
                <select
                  id={`block-${String(block.key)}-layout`}
                  value={block.layout}
                  onChange={(event) => {
                    onChange(updateBlock(blocks, block.key, { layout: event.target.value }));
                  }}
                >
                  {blockLayoutOptions(block.layout).map((option) => (
                    <option key={option.name} value={option.name}>
                      {t(option.labelKey, { name: option.name })}
                    </option>
                  ))}
                </select>
              </p>
              <p>
                <label htmlFor={`block-${String(block.key)}-data`}>
                  {t('blocks.data', { position })}
                </label>
                <textarea
                  id={`block-${String(block.key)}-data`}
                  rows={4}
                  value={block.data}
                  onChange={(event) => {
                    onChange(updateBlock(blocks, block.key, { data: event.target.value }));
                  }}
                />
              </p>
              <p>
                <label htmlFor={`block-${String(block.key)}-layout-data`}>
                  {t('blocks.layoutData', { position })}
                </label>
                <textarea
                  id={`block-${String(block.key)}-layout-data`}
                  rows={2}
                  value={block.layoutData}
                  onChange={(event) => {
                    onChange(updateBlock(blocks, block.key, { layoutData: event.target.value }));
                  }}
                />
              </p>
              <p>
                <button
                  type="button"
                  aria-label={t('blocks.moveUp', { position })}
                  disabled={index === 0}
                  onClick={() => {
                    onChange(moveBlock(blocks, block.key, -1));
                  }}
                >
                  {t('blocks.moveUpText')}
                </button>{' '}
                <button
                  type="button"
                  aria-label={t('blocks.moveDown', { position })}
                  disabled={index === blocks.length - 1}
                  onClick={() => {
                    onChange(moveBlock(blocks, block.key, 1));
                  }}
                >
                  {t('blocks.moveDownText')}
                </button>{' '}
                <button
                  type="button"
                  aria-label={t('blocks.removeLabel', { position })}
                  onClick={() => {
                    onChange(removeBlock(blocks, block.key));
                  }}
                >
                  {t('blocks.remove')}
                </button>
              </p>
            </li>
          );
        })}
      </ul>
      <p>
        <button
          type="button"
          onClick={() => {
            onChange(addBlock(blocks));
          }}
        >
          {t('blocks.add')}
        </button>
      </p>
    </fieldset>
  );
};
