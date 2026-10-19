/**
 * The draft at its edges: a stored page read in, the page and block writes
 * sent out, and a template's blocks read in and written back.
 *
 * The reads are lossless where upstream is narrow. A key the registry does not
 * name, a layout it does not register and a block's attachments all survive
 * `fromApi` and `toApi` unchanged, because `docs/compat.md` promises that an
 * upstream save ignores the extra rather than losing it.
 */
import type {
  SiteBlockAttachmentWrite,
  SiteBlockWrite,
  SiteId,
  SitePage,
  SitePageWrite,
} from '../../../api/types';
import type { BlockRegistry } from '../blockSchema';
import type { SavedTemplate, TemplateBlock } from '../templates';
import { blockKey } from './draft';
import type { BlockDraft, PageContent, PageDraft } from './draft';
import { canonicalBlocks, isGroup, spanOf } from './groups';
import { withoutPlainHash } from './romeka';
import { dataObject, readData, readLayoutData } from './values';

/** A block from a template: a fresh key and no attachments. */
export const fromTemplateBlock = (index: number, block: TemplateBlock): BlockDraft => ({
  key: blockKey(index),
  layout: block.layout,
  data: readData(block.data),
  layoutData: readLayoutData(block.layoutData),
  attachments: [],
});

/**
 * Reads a stored page. Unknown data and layout-data keys are kept verbatim, an
 * unknown layout keeps its payload, and a group span is clamped to the blocks
 * it can own.
 */
export const fromApi = (page: SitePage, registry: BlockRegistry): PageDraft => {
  const content: PageContent = {
    title: page.title,
    slug: page.slug,
    isPublic: page.isPublic,
    layout: page.layout === 'grid' ? 'grid' : 'normal',
    layoutData: readData(page.layoutData),
    blocks: canonicalBlocks(
      page.blocks.map((block, index) => ({
        key: blockKey(index),
        layout: block.layout,
        data: readData(block.data),
        layoutData: readLayoutData(block.layoutData),
        attachments: block.attachments.map((entry) => ({
          item: entry.item === null ? null : entry.item.id,
          media: entry.media === null ? null : entry.media.id,
          caption: entry.caption,
        })),
      })),
    ),
  };
  return {
    ...content,
    saved: content,
    past: [],
    future: [],
    nextKey: content.blocks.length,
    registry,
    edit: null,
  };
};

/** The `o:block` writes for a draft. A group's span is written as upstream's string. */
export const toApi = (draft: PageContent): readonly SiteBlockWrite[] =>
  canonicalBlocks(draft.blocks).map((block) => ({
    layout: block.layout,
    data: isGroup(block) ? { ...dataObject(block), span: String(spanOf(block)) } : block.data,
    layoutData: block.layoutData,
    attachments: block.attachments.map(
      (entry): SiteBlockAttachmentWrite => ({
        item: entry.item,
        media: entry.media,
        caption: entry.caption,
      }),
    ),
  }));

/** The whole page write, which `api.createSitePage` and `api.updateSitePage` take. */
export const toPageWrite = (draft: PageContent, siteId: SiteId): SitePageWrite => ({
  site: siteId,
  title: draft.title,
  slug: draft.slug,
  isPublic: draft.isPublic,
  layout: draft.layout === 'grid' ? 'grid' : null,
  layoutData: draft.layoutData,
  blocks: toApi(draft),
});

/**
 * The draft as a saved template. The blocks are canonical, carry no attachments
 * and no carrier plain hash, so the next save regenerates the Romeka rendering.
 */
export const toSavedTemplate = (
  draft: PageContent,
  id: string,
  name: string,
  description: string,
): SavedTemplate => ({
  id,
  name,
  description,
  layout: draft.layout,
  layoutData: draft.layoutData,
  blocks: canonicalBlocks(draft.blocks).map((block) => ({
    layout: block.layout,
    data: block.data,
    layoutData: block.layoutData === null ? null : withoutPlainHash(block.layoutData),
  })),
});
