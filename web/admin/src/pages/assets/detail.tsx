import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Asset, AssetId } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { assetKind, fileImageUrl } from '../../app/media';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { AssetEditForm } from './form';
import { AssetTile } from './upload';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/**
 * `Admin/AssetController::showDetailsAction` with `::editAction` and
 * `::deleteAction`: one asset, the file it stored, the edit form and the
 * delete control. The preview follows the file's kind, so a PDF or a recording
 * shows its kind's tile and its file link instead of a broken image.
 */
export const AssetDetail = ({ assetId }: { readonly assetId: AssetId }): ReactElement => {
  const { t } = useTranslator(common, admin);
  const state = useAsync(() => api.getAsset(assetId), [assetId]);
  const [edited, setEdited] = useState<Asset | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.asset')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const asset = edited ?? state.data;
  const kind = assetKind(asset);
  const imageUrl = fileImageUrl(asset.mediaType, asset.assetUrl);
  return (
    <section>
      <h2>{asset.name}</h2>

      {imageUrl === null ? (
        <p>
          <AssetTile asset={asset} className="asset-preview asset-preview-kind" decorative={false} />
        </p>
      ) : (
        <img className="asset-preview" src={imageUrl} alt={asset.altText ?? asset.name} />
      )}

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{asset.id}</dd>
        <dt>{t('column.name')}</dt>
        <dd>{asset.name}</dd>
        <dt>{t('column.filename')}</dt>
        <dd>{asset.filename}</dd>
        <dt>{t('column.mediaType')}</dt>
        <dd>{asset.mediaType}</dd>
        <dt>{t(kind === 'image' ? 'column.altText' : 'assets.linkText')}</dt>
        <dd>{asset.altText ?? t('common.none')}</dd>
        <dt>{t('column.owner')}</dt>
        <dd>
          {asset.owner === null ? (
            t('common.noOwner')
          ) : (
            <a href={`#/users/${String(asset.owner.id)}`}>{`#${String(asset.owner.id)}`}</a>
          )}
        </dd>
        <dt>{t('asset.file')}</dt>
        <dd>
          <a href={asset.assetUrl}>{asset.filename}</a>
        </dd>
      </dl>

      <EditPanel label={t('assets.editLabel')}>
        <AssetEditForm asset={asset} onSaved={setEdited} />
      </EditPanel>

      <DeleteControl
        label={t('assets.deleteLabel')}
        rowName={asset.name}
        onConfirm={async () => {
          await api.deleteAsset(asset.id);
          window.location.hash = '#/assets';
        }}
      />
    </section>
  );
};
