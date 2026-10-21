/**
 * What the Theme page previews: the site's homepage, or a sample item, item
 * set or media page, so a change to the resource page sections shows on a
 * real record. The sample is the first record of that kind on the site.
 *
 * A resource preview POSTs `resource_page: {kind, id}` with the draft theme
 * map to the same preview route (`crates/omeka-server/src/site/preview.rs`).
 */
import { useMemo } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../../api/client';
import { listMedia } from '../../../api/media';
import type { JsonValue, SiteId } from '../../../api/types';
import { useAsync } from '../../../app/useAsync';
import { useTranslator } from '../../../i18n';
import { sites } from '../../../i18n/namespaces/sites';

/** The resource names the stored `resource_page_blocks` map uses. */
export type ResourceTarget = 'items' | 'item_sets' | 'media';
export type PreviewTarget = 'page' | ResourceTarget;

const TARGETS: readonly PreviewTarget[] = ['page', 'items', 'item_sets', 'media'];

/** What the empty preview says when the site has no record of the kind. */
export const NONE_KEYS = {
  items: 'themePreview.noneItems',
  item_sets: 'themePreview.noneItem_sets',
  media: 'themePreview.noneMedia',
} as const;

const firstId = async (target: ResourceTarget, siteId: SiteId): Promise<number | null> => {
  const query = { siteId, perPage: 1 };
  const page =
    target === 'items'
      ? await api.listItems(query)
      : target === 'item_sets'
        ? await api.listItemSets(query)
        : await listMedia(api, query);
  return page.items[0]?.id ?? null;
};

/** A sample record to preview: loading, none on the site, or its id. */
export type Sample =
  | { readonly status: 'loading' }
  | { readonly status: 'none'; readonly kind: ResourceTarget }
  | { readonly status: 'ready'; readonly kind: ResourceTarget; readonly id: number };

/** The sample record for a resource target, `null` for the page target. */
export const useSample = (siteId: SiteId, target: PreviewTarget): Sample | null => {
  const state = useAsync(
    async () => (target === 'page' ? null : firstId(target, siteId)),
    [siteId, target],
  );
  const found = state.status === 'ready' ? state.data : undefined;
  // One object per answer, so the preview request that depends on it fires once.
  return useMemo((): Sample | null => {
    if (target === 'page') {
      return null;
    }
    if (found === undefined) {
      return { status: 'loading' };
    }
    return found === null
      ? { status: 'none', kind: target }
      : { status: 'ready', kind: target, id: found };
  }, [target, found]);
};

/** The preview body for a sample record under the draft theme map. */
export const resourcePreviewBody = (
  sample: { readonly kind: ResourceTarget; readonly id: number },
  settings: Readonly<Record<string, JsonValue>>,
): Readonly<Record<string, unknown>> => ({
  resource_page: { kind: sample.kind, id: sample.id },
  'o:theme_settings': settings,
});

/** The "Preview: page / an item / an item set / a media" choice. */
export const PreviewSwitch = ({
  value,
  onChange,
}: {
  readonly value: PreviewTarget;
  readonly onChange: (target: PreviewTarget) => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  return (
    <label className="theme-preview-target">
      {t('themePreview.label')}
      <select
        value={value}
        onChange={(event) => {
          const next = TARGETS.find((target) => target === event.target.value);
          if (next !== undefined) {
            onChange(next);
          }
        }}
      >
        {TARGETS.map((target) => (
          <option key={target} value={target}>
            {t(`themePreview.${target}`)}
          </option>
        ))}
      </select>
    </label>
  );
};
