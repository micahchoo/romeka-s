/**
 * The site check: what a visitor meets on a site's public pages that a curator
 * may not mean them to, each line an exact count from the stored pages and the
 * server's answers, with the pages it is on. A private page reaches no
 * visitor, so it counts in no line. A private site reaches no visitor at all;
 * the check says so and still counts its lines, which hold once it is public.
 *
 * The hub's check panel loads this module and runs it only when the curator
 * presses the check, never on hub load: a list block costs one request. Lists
 * with the same query share their request, and at most
 * `LIST_REQUESTS_AT_ONCE` are in flight. The items and media the blocks show
 * by id are read in one request per kind (`ID_BATCH` ids each).
 */
import { api } from '../../api/client';
import { listMedia } from '../../api/media';
import type { ItemId, MediaId, PageId, SiteId, SitePage, SitePageBlock } from '../../api/types';
import { BLOCK_REGISTRY } from './blockRegistry';
import { blockList, blockRefs } from './editor/blockFields';
import type { ResourceRef } from './editor/blockFields';
import { listsAttachedOnly, readVisitorCount, readVisitorMediaOfPrivateItems } from './editor/fields/listReach';
import type { ListSite } from './editor/fields/listReach';
import { readLayoutData } from './editor/values';
import { unwrittenField } from './editor/written';
import type { BlockContent } from './editor/written';
import { homepageOf, linkedPages } from './navigationTree';

/** A page a line points at. */
export interface CheckPage {
  readonly id: PageId;
  readonly title: string;
}

/**
 * What a line counts. `unwritten`, `emptyLists` and `privateResources` count
 * blocks on public pages; `outsideMenu` counts public pages.
 */
export type CheckKind = 'unwritten' | 'emptyLists' | 'privateResources' | 'outsideMenu';

/** One line of the check: a count above zero and the pages it is on. */
export interface CheckLine {
  readonly kind: CheckKind;
  readonly count: number;
  readonly pages: readonly CheckPage[];
}

/** The check's answer: whether visitors reach the site at all, and its lines. */
export interface SiteCheckResult {
  readonly sitePublic: boolean;
  readonly lines: readonly CheckLine[];
}

const PAGES_PER_REQUEST = 200;
const LIST_REQUESTS_AT_ONCE = 4;
const ID_BATCH = 100;

/** A block of a public page, as the check reads it. */
interface Found {
  readonly page: CheckPage;
  readonly block: BlockContent;
  readonly attachments: readonly ResourceRef[];
}

const foundOn = (page: SitePage, block: SitePageBlock): Found => ({
  page: { id: page.id, title: page.title },
  block: { layout: block.layout, data: block.data, layoutData: readLayoutData(block.layoutData) },
  attachments: block.attachments.map((entry) => ({
    item: entry.item?.id ?? null,
    media: entry.media?.id ?? null,
  })),
});

/** Every page of the site, in the site's own order. */
const readPages = async (siteId: SiteId, order: readonly PageId[]): Promise<readonly SitePage[]> => {
  const pages: SitePage[] = [];
  for (let page = 1; ; page += 1) {
    const answer = await api.listSitePages({ siteId, perPage: PAGES_PER_REQUEST, page });
    pages.push(...answer.items);
    if (page >= answer.lastPage || answer.items.length === 0) {
      break;
    }
  }
  const rank = new Map(order.map((id, index) => [id, index]));
  return pages.sort((a, b) => (rank.get(a.id) ?? Infinity) - (rank.get(b.id) ?? Infinity));
};

/** `run` over each value, at most `limit` at once, results in value order. */
const mapLimited = async <T, R>(
  values: readonly T[],
  limit: number,
  run: (value: T) => Promise<R>,
): Promise<readonly R[]> => {
  const results = new Map<number, R>();
  let next = 0;
  const worker = async (): Promise<void> => {
    for (let index = next++; index < values.length; index = next++) {
      const value = values[index];
      if (value !== undefined) {
        results.set(index, await run(value));
      }
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, values.length) }, worker));
  return values.flatMap((_, index) => {
    const result = results.get(index);
    return result === undefined ? [] : [result];
  });
};

const chunks = <T>(values: readonly T[]): readonly (readonly T[])[] =>
  Array.from({ length: Math.ceil(values.length / ID_BATCH) }, (_, index) =>
    values.slice(index * ID_BATCH, (index + 1) * ID_BATCH),
  );

/** What a visitor meets in one list: how many resources, and of those media how many belong to a private item. */
interface ListShown {
  readonly count: number;
  readonly mediaOfPrivateItems: number;
}

/**
 * The written list blocks a visitor meets nothing in (`empty`), and those that
 * show a visitor media of a private item (`privateMedia`). Each distinct list
 * costs one request, and a media list that shows something one more.
 */
const listsShown = async (
  site: ListSite,
  blocks: readonly Found[],
): Promise<{ readonly empty: readonly Found[]; readonly privateMedia: readonly Found[] }> => {
  const lists = blocks.flatMap((found) => {
    const list = blockList(BLOCK_REGISTRY, found.block);
    return list === null ? [] : [{ found, key: `${list.resource}\n${list.query}`, list }];
  });
  const distinct = [...new Map(lists.map((entry) => [entry.key, entry.list])).entries()];
  const shown = new Map(
    await mapLimited(distinct, LIST_REQUESTS_AT_ONCE, async ([key, list]): Promise<readonly [string, ListShown]> => {
      const count = await readVisitorCount(list.resource, list.query, site);
      const mediaOfPrivateItems =
        list.resource === 'media' && count > 0 ? await readVisitorMediaOfPrivateItems(list.query, site) : 0;
      return [key, { count, mediaOfPrivateItems }];
    }),
  );
  return {
    empty: lists.filter((entry) => shown.get(entry.key)?.count === 0).map((entry) => entry.found),
    privateMedia: lists
      .filter((entry) => (shown.get(entry.key)?.mediaOfPrivateItems ?? 0) > 0)
      .map((entry) => entry.found),
  };
};

/**
 * The written blocks that name an item or media a visitor cannot see: a
 * private item, a private media, or a media of a private item. An id the
 * server no longer has is not counted. A media list showing media of a private
 * item counts in the same line (`listsShown`).
 */
const privateResources = async (blocks: readonly Found[]): Promise<readonly Found[]> => {
  const refs = blocks.map((found) => ({ found, refs: blockRefs(BLOCK_REGISTRY, found.block, found.attachments) }));
  const all = refs.flatMap((entry) => entry.refs);
  const mediaIds = [...new Set(all.flatMap((entry) => (entry.media === null ? [] : [entry.media])))];
  const media = new Map<MediaId, { readonly isPublic: boolean; readonly item: ItemId | null }>();
  for (const batch of chunks(mediaIds)) {
    const answer = await listMedia(api, { 'id[]': batch.map(String), perPage: batch.length });
    for (const entry of answer.items) {
      media.set(entry.id, { isPublic: entry.isPublic, item: entry.item?.id ?? null });
    }
  }
  const itemIds = [
    ...new Set([
      ...all.flatMap((entry) => (entry.item === null ? [] : [entry.item])),
      ...[...media.values()].flatMap((entry) => (entry.item === null ? [] : [entry.item])),
    ]),
  ];
  const privateItems = new Set<ItemId>();
  for (const batch of chunks(itemIds)) {
    const answer = await api.listItems({ 'id[]': batch.map(String), is_public: 0, perPage: batch.length });
    for (const entry of answer.items) {
      privateItems.add(entry.id);
    }
  }
  const hidden = (ref: ResourceRef): boolean => {
    const file = ref.media === null ? undefined : media.get(ref.media);
    return (
      (ref.item !== null && privateItems.has(ref.item)) ||
      (file !== undefined && (!file.isPublic || (file.item !== null && privateItems.has(file.item))))
    );
  };
  return refs.filter((entry) => entry.refs.some(hidden)).map((entry) => entry.found);
};

/** The line for `found`: one per block, its pages each once, in site order. */
const lineOf = (kind: CheckKind, found: readonly Pick<Found, 'page'>[]): CheckLine => ({
  kind,
  count: found.length,
  pages: [...new Map(found.map((entry) => [entry.page.id, entry.page])).values()],
});

/** Reads the site and its pages and answers whether it is public, and its lines with a count above zero. */
export const readSiteCheck = async (siteId: SiteId): Promise<SiteCheckResult> => {
  const site = await api.getSite(siteId);
  const pages = (await readPages(siteId, site.pages.map((ref) => ref.id))).filter((page) => page.isPublic);
  const blocks = pages.flatMap((page) => page.blocks.map((block) => foundOn(page, block)));
  const unwritten = blocks.filter((found) => unwrittenField(BLOCK_REGISTRY, found.block) !== null);
  // An unwritten block reaches no visitor, so it counts in its own line only.
  const written = blocks.filter((found) => unwrittenField(BLOCK_REGISTRY, found.block) === null);
  const [lists, hidden] = await Promise.all([
    listsShown({ id: siteId, attachedOnly: listsAttachedOnly(site.siteSettings) }, written),
    privateResources(written),
  ]);
  // A block that both names a private record and lists media of a private item counts once.
  const pointsToPrivate = [...new Set([...hidden, ...lists.privateMedia])];
  const homepage = homepageOf(site);
  const inMenu = linkedPages(site.navigation);
  const outside = pages
    .filter((page) => page.id !== homepage && !inMenu.has(page.id))
    .map((page) => ({ page: { id: page.id, title: page.title } }));
  return {
    sitePublic: site.isPublic,
    lines: [
      lineOf('unwritten', unwritten),
      lineOf('emptyLists', lists.empty),
      lineOf('privateResources', written.filter((found) => pointsToPrivate.includes(found))),
      lineOf('outsideMenu', outside),
    ].filter((line) => line.count > 0),
  };
};
