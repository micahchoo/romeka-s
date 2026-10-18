import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, Asset } from '../../api/types';
import { ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 25;

interface AssetRow {
  readonly asset: Asset;
  readonly ownerName: string | null;
}

const load = async (page: number): Promise<ApiPage<AssetRow>> => {
  const [assets, users] = await Promise.all([
    api.listAssets({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listUsers({ perPage: 100 }),
  ]);
  const names = userNames(users.items);
  return mapPage(assets, (asset) => ({ asset, ownerName: ownerNameOf(names, asset.owner) }));
};

/** `Admin/AssetController::browseAction`: the asset browse table. */
export const AssetList = (): ReactElement => {
  const { t } = useTranslator();
  const resource = t('resource.assets');
  return (
    <ListPage<AssetRow>
      resourceLabel={resource}
      emptyMessage={t('list.empty', { resource })}
      load={load}
      rowKey={(row) => String(row.asset.id)}
      columns={[
        {
          header: t('column.name'),
          cell: (row) => <a href={`#/assets/${String(row.asset.id)}`}>{row.asset.name}</a>,
        },
        { header: t('column.filename'), cell: (row) => row.asset.filename },
        { header: t('column.mediaType'), cell: (row) => row.asset.mediaType },
        { header: t('column.owner'), cell: (row) => row.ownerName ?? t('common.noOwner') },
        { header: t('column.altText'), cell: (row) => row.asset.altText ?? '' },
      ]}
    />
  );
};
