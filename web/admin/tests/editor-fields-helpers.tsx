/**
 * Shared builders for the block field tests: a loaded block, its schema, a
 * dispatch spy, and the wire shapes the tests hand the draft model.
 */
import { vi } from 'vitest';
import type { Mock } from 'vitest';
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { SiteId } from '../src/api/types';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { BlockLayoutSchema } from '../src/pages/sites/blockSchema';
import type { BlockDraft, BlockKey, DraftAction, PageDraft, ValidationIssue } from '../src/pages/sites/editor/draft';
import { validate } from '../src/pages/sites/editor/draft';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { BlockForm } from '../src/pages/sites/editor/fields/BlockForm';
import type { EditorOptions } from '../src/pages/sites/editor/options';
import { schemaFor } from '../src/pages/sites/editor/schema';
import { find, load, parsePage, rawBlock } from './editor-draft-helpers';

/** The site every block form test edits. */
export const SITE_ID: SiteId = parsePage([]).site.id;

/** One stored block, loaded through the wire parser. */
export const draftFor = (
  layout: string,
  data: unknown,
  extra: Readonly<Record<string, unknown>> = {},
): BlockDraft => find(load([rawBlock(layout, data, extra)]), layout);

/** A layout's schema, which every registry layout has. */
export const schemaForLayout = (layout: string): BlockLayoutSchema => {
  const schema = schemaFor(BLOCK_REGISTRY, layout);
  if (schema === undefined) {
    throw new Error(`the registry does not name ${layout}`);
  }
  return schema;
};

export type DispatchSpy = Mock<(action: DraftAction) => void>;

export const dispatchSpy = (): DispatchSpy => vi.fn<(action: DraftAction) => void>();

/** Every action a spy recorded, in call order. */
export const actionsOf = (spy: DispatchSpy): readonly DraftAction[] =>
  spy.mock.calls.map((call) => call[0]);

/** The last action a spy recorded. */
export const lastAction = (spy: DispatchSpy): DraftAction => {
  const call = spy.mock.calls[spy.mock.calls.length - 1];
  if (call === undefined) {
    throw new Error('expected at least one dispatched action');
  }
  return call[0];
};

/** One `o:attachment` wire entry. */
export const rawAttachment = (
  item: number | null,
  media: number | null,
  caption: string,
): unknown => ({
  'o:item': item === null ? null : { '@id': `http://localhost/api/items/${String(item)}`, 'o:id': item },
  'o:media':
    media === null ? null : { '@id': `http://localhost/api/media/${String(media)}`, 'o:id': media },
  'o:caption': caption,
});

/** Selects everything inside `element`, so a toolbar command has a range. */
export const selectAll = (element: Element): void => {
  const range = document.createRange();
  range.selectNodeContents(element);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
};

export interface BlockFormHarnessProps {
  readonly layout: string;
  readonly data: unknown;
  readonly extra?: Readonly<Record<string, unknown>>;
  readonly issues?: readonly ValidationIssue[];
  readonly onAction?: (action: DraftAction) => void;
  /** Called with the block on every render, so a test can name its key. */
  readonly onBlock?: (block: BlockDraft) => void;
  /** The editor's shared lists, when the test supplies them. */
  readonly options?: EditorOptions;
}

/**
 * A block form over a live draft. It applies every action through the reducer,
 * so a test sees the re-render the real editor produces and can assert on both
 * the action and the state it leads to.
 */
export const BlockFormHarness = ({
  layout,
  data,
  extra,
  issues,
  onAction,
  onBlock,
  options,
}: BlockFormHarnessProps): ReactElement => {
  const [state, setState] = useState<PageDraft>(() => load([rawBlock(layout, data, extra ?? {})]));
  const block = find(state, layout);
  onBlock?.(block);
  return (
    <BlockForm
      block={block}
      schema={schemaForLayout(layout)}
      issues={issues ?? validate(state)}
      siteId={SITE_ID}
      {...(options === undefined ? {} : { options })}
      dispatch={(action) => {
        onAction?.(action);
        setState((current) => draftReducer(current, action));
      }}
    />
  );
};

/**
 * A block key captured from a harness render. The tests assert the exact action
 * a control dispatched, and the key is part of it.
 */
export const captureBlock = (): {
  readonly onBlock: (block: BlockDraft) => void;
  readonly key: () => BlockKey;
} => {
  let captured: BlockKey | null = null;
  return {
    onBlock: (block) => {
      captured = block.key;
    },
    key: () => {
      if (captured === null) {
        throw new Error('the harness rendered no block');
      }
      return captured;
    },
  };
};
