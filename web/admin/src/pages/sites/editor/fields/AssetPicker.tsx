/**
 * The `assetRef` field control: a picker over `/api/assets` with kind tiles and
 * names, plus the shared upload control. The field says what it accepts, so an
 * image slot offers only images and the asset block offers any file. A slot
 * behind the block is decorative and asks for no description.
 *
 * The stored value is the asset id as a string, the shape upstream's asset form
 * writes, so an id the fetched page does not name still resolves by id.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../../../api/client';
import { assetIdFromParam } from '../../../../api/parse';
import type { Asset } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import type { FieldAccepts } from '../../blockSchema';
import { AssetTile, AssetUpload } from '../../../assets/upload';

/** The file kinds an `<input type="file">` offers: images for a picture slot, all files otherwise. */
const ACCEPT: Readonly<Record<FieldAccepts, string | null>> = {
  image: 'image/*',
  imageService: 'image/*',
  any: null,
};

export interface AssetPickerProps {
  readonly id: string;
  readonly label: string;
  /** One sentence under the legend that says what the asset is for. */
  readonly description?: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly accepts: FieldAccepts;
  /** Whether it sits behind the block, so it takes no description. */
  readonly decorative: boolean;
}

export const AssetPicker = ({
  id,
  label,
  description,
  value,
  onChange,
  accepts,
  decorative,
}: AssetPickerProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);
  // The grid loads on open, so a block with many asset slots makes no request
  // until someone looks at the list.
  const list = useAsync(
    () => (open ? api.listAssets({ perPage: 200, sortBy: 'name' }) : Promise.resolve(null)),
    [open, revision],
  );
  const selected = useAsync(
    (): Promise<Asset | null> => {
      const id = assetIdFromParam(value);
      return id === null ? Promise.resolve(null) : api.getAsset(id);
    },
    [value],
  );
  const assets = list.status === 'ready' && list.data !== null ? list.data.items : [];

  return (
    <fieldset
      className="block-field asset-picker"
      aria-describedby={description === undefined ? undefined : `${id}-help`}
    >
      <legend>{label}</legend>
      {description === undefined ? null : (
        <p className="setting-help" id={`${id}-help`}>
          {description}
        </p>
      )}
      <p className="asset-current">
        {selected.status === 'ready' && selected.data !== null ? (
          <>
            <AssetTile asset={selected.data} className="asset-thumb" />
            <span>{selected.data.name}</span>
          </>
        ) : selected.status === 'error' ? (
          <span>{t('asset.fallback', { id: value })}</span>
        ) : (
          <span>{value === '' ? t('asset.none') : t('asset.loading')}</span>
        )}
      </p>
      <AssetUpload
        id={id}
        accept={ACCEPT[accepts]}
        describe={!decorative}
        onAsset={(asset) => {
          onChange(String(asset.id));
          setRevision((current) => current + 1);
        }}
      />
      {value === '' ? null : (
        <p>
          <button
            type="button"
            onClick={() => {
              onChange('');
            }}
          >
            {t('action.remove')}
          </button>
        </p>
      )}
      <p>
        <button
          type="button"
          aria-expanded={open}
          onClick={() => {
            setOpen(!open);
          }}
        >
          {t('asset.choose')}
        </button>
      </p>
      {open ? (
        assets.length === 0 ? (
          <p className="asset-empty">{t('asset.empty')}</p>
        ) : (
          <ul className="asset-grid">
            {assets.map((asset) => (
              <li key={String(asset.id)}>
                <button
                  type="button"
                  aria-current={String(asset.id) === value}
                  onClick={() => {
                    onChange(String(asset.id));
                    setOpen(false);
                  }}
                >
                  <AssetTile asset={asset} className="asset-thumb" />
                  <span>{asset.name}</span>
                </button>
              </li>
            ))}
          </ul>
        )
      ) : null}
    </fieldset>
  );
};
