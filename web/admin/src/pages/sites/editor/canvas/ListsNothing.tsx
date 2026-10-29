/**
 * "Lists nothing yet" on the Outline row of a block whose stored query matches
 * nothing on the site: a browse preview, a map, a timeline. It reads the same
 * count as the inspector (`listReach.ts`), one request per list block when the
 * page opens and again when its query settles after an edit; a zero asks once
 * more for the site's own total. A block with no query field asks nothing.
 */
import type { ReactElement } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import type { BlockRegistry } from '../../blockSchema';
import type { BlockDraft } from '../draft';
import { asText } from '../fields/controlBase';
import { isQueryResource, listsAttachedOnly, useListReach } from '../fields/listReach';
import type { QueryResource } from '../fields/listReach';
import { useEditorText } from '../messages';
import { romkaFor, settingsOf } from '../romeka';
import { schemaFor } from '../schema';
import { dataObject } from '../values';

/** The query a block lists by and the resource it searches, `null` for a block that lists nothing by query. */
export const blockList = (
  registry: BlockRegistry,
  block: BlockDraft,
): { readonly resource: QueryResource; readonly query: string } | null => {
  const romka = romkaFor(registry, block);
  const fields = romka?.fields ?? schemaFor(registry, block.layout)?.fields ?? [];
  const record = romka === undefined ? dataObject(block) : settingsOf(block);
  for (const field of fields) {
    if (field.type === 'itemQuery') {
      const resource = field.resourceKey === null ? 'items' : record[field.resourceKey];
      return {
        resource: isQueryResource(resource) ? resource : 'items',
        query: asText(record[field.key]),
      };
    }
  }
  return null;
};

export const ListsNothing = ({
  registry,
  block,
  siteId,
  siteSettings,
}: {
  readonly registry: BlockRegistry;
  readonly block: BlockDraft;
  readonly siteId: SiteId;
  readonly siteSettings: JsonValue | null;
}): ReactElement | null => {
  const e = useEditorText();
  const list = blockList(registry, block);
  const reach = useListReach(list?.resource ?? 'items', list?.query ?? null, {
    id: siteId,
    attachedOnly: listsAttachedOnly(siteSettings),
  });
  return reach.status === 'ready' && reach.data !== null && reach.data.kind !== 'matches' ? (
    <span className="canvas-badge canvas-badge-quiet">{e('canvas.listsNothing')}</span>
  ) : null;
};
