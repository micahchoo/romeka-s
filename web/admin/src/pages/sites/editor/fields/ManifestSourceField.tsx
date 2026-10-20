/**
 * The `manifestSource` field control: the IIIF manifest a viewer opens, given
 * as a web address or as an item of this site. Either way the block stores one
 * URL, so upstream opens the same manifest; an item is stored as the manifest
 * this server publishes for it (`/iiif/3/item/<id>/manifest`), and a stored URL
 * of that shape on this origin reads back as that item.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../../../api/client';
import { brandId } from '../../../../api/parse';
import type { Item, ItemId, JsonValue } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { editor } from '../../../../i18n/namespaces/editor';
import { Help, helpId } from '../Labelled';
import { ItemPicker } from './ItemPicker';
import './controls.css';

type Source = 'url' | 'item';

/** The manifest this server publishes for one item. */
export const itemManifestUrl = (origin: string, item: ItemId): string =>
  `${origin}/iiif/3/item/${String(item)}/manifest`;

/** The item a stored manifest URL names, `null` for any other address. */
export const manifestItem = (origin: string, url: string): ItemId | null => {
  const prefix = `${origin}/iiif/3/item/`;
  if (!url.startsWith(prefix) || !url.endsWith('/manifest')) {
    return null;
  }
  const id = url.slice(prefix.length, -'/manifest'.length);
  return /^[1-9]\d*$/.test(id) ? brandId<'ItemId'>(Number(id)) : null;
};

/** The chosen item's thumbnail and title. */
const ChosenItem = ({ item }: { readonly item: ItemId }): ReactElement => {
  const { t } = useTranslator(common, editor);
  const state = useAsync((): Promise<Item> => api.getItem(item), [item]);
  const found = state.status === 'ready' ? state.data : null;
  const thumbnail = found?.thumbnailDisplayUrls.square;
  return (
    <p className="attachment-item">
      {thumbnail === undefined ? (
        <span className="attachment-thumb" aria-hidden="true" />
      ) : (
        <img className="attachment-thumb" src={thumbnail} alt="" />
      )}
      <strong>
        {found !== null
          ? (found.title ?? t('common.untitled'))
          : t('manifest.itemFallback', { id: item })}
      </strong>
    </p>
  );
};

export interface ManifestSourceFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
  /** The origin the item manifests are published on. */
  readonly origin?: string;
}

export const ManifestSourceField = ({
  id,
  label,
  help,
  value,
  onChange,
  origin = window.location.origin,
}: ManifestSourceFieldProps): ReactElement => {
  const { t } = useTranslator(editor);
  const url = typeof value === 'string' ? value : '';
  const item = manifestItem(origin, url);
  const [source, setSource] = useState<Source>(item === null ? 'url' : 'item');
  const choose = (next: Source): void => {
    setSource(next);
    if (url !== '' && (next === 'item') !== (item !== null)) {
      onChange('');
    }
  };
  return (
    <fieldset className="block-field link-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      <div className="link-kinds">
        {(['url', 'item'] as const).map((option) => (
          <label key={option} className="block-check">
            <input
              type="radio"
              name={`${id}-source`}
              value={option}
              checked={source === option}
              onChange={() => {
                choose(option);
              }}
            />
            {t(option === 'url' ? 'manifest.fromUrl' : 'manifest.fromItem')}
          </label>
        ))}
      </div>
      {source === 'url' ? (
        <input
          id={id}
          type="url"
          aria-label={t('manifest.fromUrl')}
          placeholder={t('fieldExample.url')}
          value={url}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      ) : (
        <>
          {item === null ? null : <ChosenItem item={item} />}
          <ItemPicker
            id={`${id}-picker`}
            label={t('manifest.pickItem')}
            picked={item === null ? [] : [item]}
            onChange={(picked) => {
              const chosen = picked.find((entry) => entry !== item);
              onChange(chosen === undefined ? '' : itemManifestUrl(origin, chosen));
            }}
          />
        </>
      )}
      <Help id={id} text={help} />
    </fieldset>
  );
};
