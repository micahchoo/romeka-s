import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { installation } from '../../app/installation';
import type { ApiPage, Asset } from '../../api/types';
import { ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { KIND_LABEL, assetKind } from '../../app/media';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';
import { resources } from '../../i18n/namespaces/resources';
import { AssetTile, AssetUpload } from './upload';

interface AssetRow {
  readonly asset: Asset;
  readonly ownerName: string | null;
}

const load = async (page: number): Promise<ApiPage<AssetRow>> => {
  const [assets, users] = await Promise.all([
    api.listAssets({ page, perPage: installation().perPage, sortBy: 'id', sortOrder: 'asc' }),
    api.listUsers({ perPage: 100 }),
  ]);
  const names = userNames(users.items);
  return mapPage(assets, (asset) => ({ asset, ownerName: ownerNameOf(names, asset.owner) }));
};

/**
 * `Admin/AssetController::browseAction`: the asset browse table, with the
 * upload that creates one above it. Assets have no create page, so this table
 * is the one place an asset enters the archive.
 */
export const AssetList = (): ReactElement => {
  const { t } = useTranslator(common, admin, resources);
  const resource = t('resource.assets');
  const [revision, setRevision] = useState(0);
  return (
    <section>
      <AssetUpload
        id="asset-list-upload"
        accept={null}
        describe
        onAsset={() => {
          setRevision((current) => current + 1);
        }}
      />
      <ListPage<AssetRow>
        key={revision}
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.asset.id)}
        columns={[
          {
            header: t('assets.columnKind'),
            cell: (row) => (
              <span className="asset-kind-cell">
                <AssetTile asset={row.asset} className="asset-kind-tile" compact />
                <span>{t(KIND_LABEL[assetKind(row.asset)])}</span>
              </span>
            ),
          },
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
    </section>
  );
};
