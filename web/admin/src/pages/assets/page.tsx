import type { ReactElement } from 'react';
import { assetIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { AssetDetail } from './detail';
import { AssetList } from './list';
import { admin } from '../../i18n/namespaces/admin';

/** `#/assets` lists; `#/assets/<id>` shows one asset. There is no create route. */
export const AssetsPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator(admin);
  if (param === null) {
    return <AssetList />;
  }
  const assetId = assetIdFromParam(param);
  if (assetId === null) {
    return <p role="alert">{t('route.invalidAssetId', { param })}</p>;
  }
  return <AssetDetail key={String(assetId)} assetId={assetId} />;
};
