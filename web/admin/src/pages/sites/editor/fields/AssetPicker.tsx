/**
 * The `assetRef` field control: a picker over `/api/assets` with thumbnails and
 * names, plus an upload. `POST /api/assets` is multipart (a JSON `data` part and
 * a `file` part), which the browser's `FormData` builds; `api.createAsset` owns
 * the request, and this control owns the list and the error state.
 *
 * The stored value is the asset id as a string, the shape upstream's asset form
 * writes, so an id the fetched page does not name still resolves by id.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../../../api/client';
import { assetIdFromParam } from '../../../../api/parse';
import type { Asset } from '../../../../api/types';
import { formatError } from '../../../../app/format';
import { toError, useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';

export interface AssetPickerProps {
  readonly id: string;
  readonly label: string;
  /** One sentence under the legend that says what the asset is for. */
  readonly description?: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly required?: boolean;
}

export const AssetPicker = ({
  id,
  label,
  description,
  value,
  onChange,
  required = false,
}: AssetPickerProps): ReactElement => {
  const translator = useTranslator(common, sites);
  const { t } = translator;
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
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

  const upload = (file: File, input: HTMLInputElement): void => {
    input.value = '';
    setBusy(true);
    setError(null);
    api.createAsset({ name: file.name, altText: null }, file).then(
      (asset) => {
        setBusy(false);
        onChange(String(asset.id));
        setRevision((current) => current + 1);
      },
      (cause: unknown) => {
        setBusy(false);
        setError(formatError(toError(cause), translator));
      },
    );
  };

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
            <img src={selected.data.assetUrl} alt="" className="asset-thumb" />
            <span>{selected.data.name}</span>
          </>
        ) : selected.status === 'error' ? (
          <span>{t('asset.fallback', { id: value })}</span>
        ) : (
          <span>{value === '' ? t('asset.none') : t('asset.loading')}</span>
        )}
      </p>
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
        <label htmlFor={`${id}-upload`}>{t('asset.upload')}</label>
        <input
          id={`${id}-upload`}
          type="file"
          accept="image/*"
          disabled={busy}
          aria-required={required}
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file !== undefined) {
              upload(file, event.currentTarget);
            }
          }}
        />
        {value === '' ? null : (
          <button
            type="button"
            onClick={() => {
              onChange('');
            }}
          >
            {t('action.remove')}
          </button>
        )}
      </p>
      {busy ? (
        <p className="asset-busy" role="status">
          {t('asset.uploading')}
        </p>
      ) : null}
      {error === null ? null : (
        <p className="field-error" role="alert">
          {t('asset.error', { message: error })}
        </p>
      )}
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
                  <img src={asset.assetUrl} alt="" className="asset-thumb" />
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
