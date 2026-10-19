/**
 * Shared builders for the draft tests: a wire page, a loaded draft, and the
 * small readers the assertions use.
 */
import { parseSitePage } from '../src/api/parse';
import type { JsonValue } from '../src/api/types';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { dataObject } from '../src/pages/sites/editor/values';
import { fromApi } from '../src/pages/sites/editor/wire';
import type {
  BlockAttachment,
  BlockDraft,
  BlockKey,
  DraftAction,
  PageDraft,
} from '../src/pages/sites/editor/draft';

export const at = <T>(values: readonly T[], index: number): T => {
  const value = values[index];
  if (value === undefined) {
    throw new Error(`expected an entry at ${String(index)}`);
  }
  return value;
};

export const rawBlock = (
  layout: string,
  data: unknown,
  extra: Readonly<Record<string, unknown>> = {},
): unknown => ({
  'o:layout': layout,
  'o:data': data,
  'o:layout_data': null,
  'o:attachment': [],
  ...extra,
});

export const rawPage = (
  blocks: readonly unknown[],
  extra: Readonly<Record<string, unknown>> = {},
): unknown => ({
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/site_pages/1',
  '@type': ['o:SitePage'],
  'o:id': 1,
  'o:slug': 'welcome',
  'o:title': 'Welcome',
  'o:is_public': true,
  'o:layout': null,
  'o:layout_data': [],
  'o:block': blocks,
  'o:site': { '@id': 'http://localhost/api/sites/1', 'o:id': 1 },
  'o:created': {
    '@value': '2024-01-02T10:00:00+00:00',
    '@type': 'http://www.w3.org/2001/XMLSchema#dateTime',
  },
  'o:modified': null,
  ...extra,
});

export const parsePage = (
  blocks: readonly unknown[],
  extra: Readonly<Record<string, unknown>> = {},
) => parseSitePage(rawPage(blocks, extra));

export const load = (
  blocks: readonly unknown[],
  extra: Readonly<Record<string, unknown>> = {},
): PageDraft => fromApi(parsePage(blocks, extra), BLOCK_REGISTRY);

export const apply = (state: PageDraft, ...actions: readonly DraftAction[]): PageDraft =>
  actions.reduce((current, action) => draftReducer(current, action), state);

export const dataOf = (block: BlockDraft): Readonly<Record<string, JsonValue>> =>
  dataObject(block);

export const spanOf = (block: BlockDraft): number => {
  const span = dataOf(block).span;
  if (typeof span === 'number') {
    return span;
  }
  return typeof span === 'string' ? Number(span) : 0;
};

export const find = (state: PageDraft, layout: string): BlockDraft => {
  const block = state.blocks.find((entry) => entry.layout === layout);
  if (block === undefined) {
    throw new Error(`expected a ${layout} block`);
  }
  return block;
};

export const keyOf = (state: PageDraft, layout: string): BlockKey => find(state, layout).key;

export const blockAt = (blocks: readonly BlockDraft[], index: number): BlockDraft =>
  at(blocks, index);

export const layouts = (state: PageDraft): readonly string[] =>
  state.blocks.map((block) => block.layout);

const ref = (kind: string, id: number): unknown => ({
  '@id': `http://localhost/api/${kind}/${String(id)}`,
  'o:id': id,
});

/** A typed attachment built through the wire parser, so its ids carry their brands. */
export const attachment = (
  item: number,
  media: number | null,
  caption: string,
): BlockAttachment => {
  const block = at(
    parsePage([
      rawBlock('html', {}, {
        'o:attachment': [
          {
            'o:item': ref('items', item),
            'o:media': media === null ? null : ref('media', media),
            'o:caption': caption,
          },
        ],
      }),
    ]).blocks,
    0,
  );
  const entry = at(block.attachments, 0);
  return {
    item: entry.item === null ? null : entry.item.id,
    media: entry.media === null ? null : entry.media.id,
    caption: entry.caption,
  };
};

/** A deterministic PRNG so a failing property case replays exactly. */
export const seeded = (seed: number): (() => number) => {
  let value = seed;
  return () => {
    value = (value * 1664525 + 1013904223) % 4294967296;
    return value / 4294967296;
  };
};
