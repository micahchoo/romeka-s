/**
 * "Lists nothing yet" on the Outline row of a block whose stored query matches
 * nothing on the site: a browse preview, a map, a timeline. It reads the same
 * count as the inspector (`listReach.ts`), one request per list block when the
 * page opens and again when its query settles after an edit; a zero asks once
 * more for the site's own total. A block with no shown query field asks nothing
 * (`blockFields.ts`).
 */
import type { ReactElement } from 'react';
import type { SiteId } from '../../../../api/types';
import type { BlockRegistry } from '../../blockSchema';
import { blockList } from '../blockFields';
import type { BlockDraft } from '../draft';
import { useListReach } from '../fields/listReach';
import { useEditorText } from '../messages';

export const ListsNothing = ({
  registry,
  block,
  siteId,
}: {
  readonly registry: BlockRegistry;
  readonly block: BlockDraft;
  readonly siteId: SiteId;
}): ReactElement | null => {
  const e = useEditorText();
  const list = blockList(registry, block);
  const reach = useListReach(list?.resource ?? 'items', list?.query ?? null, siteId);
  return reach.status === 'ready' && reach.data !== null && reach.data.kind !== 'matches' ? (
    <span className="canvas-badge canvas-badge-quiet">{e('canvas.listsNothing')}</span>
  ) : null;
};
