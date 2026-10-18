import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ApiPage } from '../../api/types';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';

type QueryResource = 'items' | 'media' | 'item_sets';

const RESOURCE_OPTIONS: readonly { readonly value: QueryResource; readonly labelKey: MessageKey }[] =
  [
    { value: 'items', labelKey: 'page.items' },
    { value: 'media', labelKey: 'page.media' },
    { value: 'item_sets', labelKey: 'page.itemSets' },
  ];

const isQueryResource = (value: string): value is QueryResource =>
  value === 'items' || value === 'media' || value === 'item_sets';

/** The plural resource name, the loading and pagination label for each type. */
const RESOURCE_KEYS: Readonly<Record<QueryResource, MessageKey>> = {
  items: 'resource.items',
  media: 'resource.media',
  item_sets: 'resource.itemSets',
};

interface QuerySpec {
  readonly resource: QueryResource;
  readonly search: string;
}

interface PreviewRow {
  readonly id: number;
  readonly title: string;
}

const loadRows = async (spec: QuerySpec, page: number): Promise<ApiPage<PreviewRow>> => {
  const query = { page, perPage: 25, search: spec.search };
  switch (spec.resource) {
    case 'items':
      return mapPage(await api.listItems(query), (item) => ({
        id: item.id,
        title: item.title ?? '',
      }));
    case 'media':
      return mapPage(await api.listMedia(query), (media) => ({
        id: media.id,
        title: media.title ?? '',
      }));
    case 'item_sets':
      return mapPage(await api.listItemSets(query), (itemSet) => ({
        id: itemSet.id,
        title: itemSet.title ?? '',
      }));
  }
};

const QueryPreview = ({ spec }: { readonly spec: QuerySpec }): ReactElement => {
  const { t } = useTranslator();
  return (
    <ListPage<PreviewRow>
      resourceLabel={t(RESOURCE_KEYS[spec.resource])}
      emptyMessage={t('query.empty')}
      load={(page) => loadRows(spec, page)}
      rowKey={(row) => String(row.id)}
      columns={[
        { header: t('column.id'), cell: (row) => row.id, numeric: true },
        { header: t('column.title'), cell: (row) => row.title || t('common.untitled') },
      ]}
    />
  );
};

/**
 * `Admin/QueryController::sidebarEditAction` plus `sidebarPreviewAction`: the
 * advanced search form and the resource list its preview returns.
 */
export const QueryPage = (): ReactElement => {
  const { t } = useTranslator();
  const [resource, setResource] = useState<QueryResource>('items');
  const [search, setSearch] = useState('');
  const [applied, setApplied] = useState<QuerySpec | null>(null);

  return (
    <section>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          setApplied({ resource, search });
        }}
      >
        <p>
          <label htmlFor="query-resource">{t('query.resourceType')}</label>
          <select
            id="query-resource"
            value={resource}
            onChange={(event) => {
              const value = event.target.value;
              if (isQueryResource(value)) {
                setResource(value);
              }
            }}
          >
            {RESOURCE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {t(option.labelKey)}
              </option>
            ))}
          </select>
        </p>
        <p>
          <label htmlFor="query-search">{t('query.search')}</label>
          <input
            id="query-search"
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
            }}
          />
        </p>
        <button type="submit">{t('action.preview')}</button>
      </form>
      {applied === null ? (
        <p>{t('query.idle')}</p>
      ) : (
        <QueryPreview key={`${applied.resource}:${applied.search}`} spec={applied} />
      )}
    </section>
  );
};
