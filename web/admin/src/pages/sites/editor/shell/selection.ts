/**
 * The one block selection. The shell owns it; the Outline and the preview only
 * read it and ask to change it.
 */
import type { BlockDraft, BlockKey, PageContent } from '../draft';

/** Who selected, so the other surface knows to reveal the block. */
export type SelectionSource = 'outline' | 'preview' | 'issue' | 'inspector';

export interface Selection {
  readonly key: BlockKey;
  readonly from: SelectionSource;
}

/** The selected block, or `null` when nothing is selected or the key is gone from the draft. */
export const blockOf = (draft: PageContent, selection: Selection | null): BlockDraft | null =>
  selection === null ? null : (draft.blocks.find((block) => block.key === selection.key) ?? null);
