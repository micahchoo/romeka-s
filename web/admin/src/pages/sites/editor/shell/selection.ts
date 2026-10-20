/**
 * The one selection. The shell owns it; the Outline and the preview only read
 * it and ask to change it. A selection is a block of the page or a site region
 * (`../regions.ts`), never both.
 */
import type { BlockDraft, BlockKey, PageContent } from '../draft';
import type { SiteRegion } from '../regions';

/** Who selected, so the other surface knows to reveal the block. */
export type SelectionSource = 'outline' | 'preview' | 'issue' | 'inspector';

/** What can be selected: a block of this page, or a part of the site frame. */
export type Subject =
  | { readonly kind: 'block'; readonly key: BlockKey }
  | { readonly kind: 'region'; readonly region: SiteRegion };

export type Selection = Subject & { readonly from: SelectionSource };

/** The selected block's key, or `null` when nothing or a region is selected. */
export const selectedKey = (selection: Subject | null): BlockKey | null =>
  selection?.kind === 'block' ? selection.key : null;

/** Whether two subjects name the same block or the same region. */
export const sameSubject = (a: Subject | null, b: Subject | null): boolean => {
  if (a === null || b === null) {
    return a === b;
  }
  return a.kind === 'block'
    ? b.kind === 'block' && a.key === b.key
    : b.kind === 'region' && a.region === b.region;
};

/** The selected block, or `null` when nothing, a region, or a key gone from the draft is selected. */
export const blockOf = (draft: PageContent, selection: Selection | null): BlockDraft | null => {
  const key = selectedKey(selection);
  return key === null ? null : (draft.blocks.find((block) => block.key === key) ?? null);
};
