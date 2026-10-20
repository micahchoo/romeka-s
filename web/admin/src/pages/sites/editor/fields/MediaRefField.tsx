/**
 * The `mediaRef` field control: one media of the site, chosen in two steps.
 * The curator finds the item with the item picker, then chooses one of its
 * images from a row of thumbnails. The stored value is the media id.
 *
 * A private media, or any media of a private item, is shown but cannot be
 * chosen, because visitors would not see it. A choice that became private
 * later stays stored, and the field says that visitors do not see it: the
 * save keeps the choice and leaves the image out of what visitors read, so an
 * unrelated edit still saves. When the picked item has exactly
 * one image that visitors can see, the control chooses it at once. Once a
 * media is chosen, the item picker folds away behind "Choose another item",
 * so the inspector shows the choice rather than the whole archive.
 */
import type { ReactElement } from 'react';
import { useEffect, useState } from 'react';
import { api } from '../../../../api/client';
import { brandId } from '../../../../api/parse';
import type { ItemId, JsonValue, Media, MediaId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { mediaName, previewImageUrl } from '../../../../app/media';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { editor } from '../../../../i18n/namespaces/editor';
import { Help, helpId } from '../Labelled';
import { LazyItemPicker } from './LazyItemPicker';
import './controls.css';

/** The media id a stored value names, `null` for none. */
export const mediaIdOf = (value: JsonValue | undefined): MediaId | null => {
  const id = typeof value === 'string' ? Number(value) : value;
  return typeof id === 'number' && Number.isInteger(id) && id > 0 ? brandId<'MediaId'>(id) : null;
};

/** The item's media that have an image to show, in the item's order. */
const imagesOf = async (item: ItemId): Promise<readonly Media[]> =>
  (await api.listMediaForItem(item, { perPage: 200 })).items.filter((media) => previewImageUrl(media) !== null);

/** The item's images, and whether visitors may see the item itself. */
const listingOf = async (
  item: ItemId | null,
): Promise<{ readonly item: ItemId | null; readonly images: readonly Media[]; readonly itemPublic: boolean }> => {
  if (item === null) {
    return { item, images: [], itemPublic: true };
  }
  const [images, record] = await Promise.all([imagesOf(item), api.getItem(item)]);
  return { item, images, itemPublic: record.isPublic };
};

export interface MediaRefFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
}

export const MediaRefField = ({ id, label, help, value, onChange }: MediaRefFieldProps): ReactElement => {
  const { t } = useTranslator(common, editor);
  const chosenId = mediaIdOf(value);
  const chosen = useAsync(
    (): Promise<Media | null> => (chosenId === null ? Promise.resolve(null) : api.getMedia(chosenId)),
    [chosenId],
  );
  const chosenMedia = chosen.status === 'ready' ? chosen.data : null;
  // The item the curator picked in this session, else the chosen media's own.
  const [picked, setPicked] = useState<ItemId | null>(null);
  const [autoChoose, setAutoChoose] = useState(false);
  const [browsing, setBrowsing] = useState(false);
  const item = picked ?? chosenMedia?.item?.id ?? null;
  const listing = useAsync(() => listingOf(item), [item]);
  // A listing still on screen from the previous item is not this item's.
  const current = listing.status === 'ready' && listing.data.item === item ? listing.data : null;
  const listed = current?.images ?? null;
  const images = listed ?? [];
  const seen = (media: Media): boolean => media.isPublic && current?.itemPublic !== false;
  const visible = images.filter(seen);
  const only = visible.length === 1 ? visible[0] : undefined;

  useEffect(() => {
    if (autoChoose && listed !== null) {
      setAutoChoose(false);
      if (only !== undefined) {
        onChange(only.id);
      }
    }
  }, [autoChoose, listed, only, onChange]);

  const thumbnail = chosenMedia === null ? null : previewImageUrl(chosenMedia);
  const chosenHidden = chosenMedia !== null && !seen(chosenMedia);
  return (
    <fieldset className="block-field media-ref-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      {chosenId === null ? null : (
        <p className="attachment-item">
          {thumbnail === null ? (
            <span className="attachment-thumb" aria-hidden="true" />
          ) : (
            <img className="attachment-thumb" src={thumbnail} alt="" />
          )}
          <strong>
            {chosenMedia === null
              ? t('mediaRef.fallback', { id: chosenId })
              : (mediaName(chosenMedia) ?? t('common.untitled'))}
          </strong>
          <button
            type="button"
            onClick={() => {
              onChange(null);
            }}
          >
            {t('mediaRef.remove')}
          </button>
        </p>
      )}
      {chosenHidden ? <p className="field-note">{t('mediaRef.privateChosen')}</p> : null}
      {item === null || listed === null ? null : images.length === 0 ? (
        <p className="field-note">{t('mediaRef.noImages')}</p>
      ) : (
        <fieldset className="media-choice">
          <legend>{t('mediaRef.choose')}</legend>
          {images.map((media) => {
            const src = previewImageUrl(media);
            const name = mediaName(media) ?? t('common.untitled');
            return (
              <label key={String(media.id)} aria-disabled={!seen(media)}>
                <input
                  type="radio"
                  name={`${id}-media`}
                  checked={chosenId === media.id}
                  disabled={!seen(media)}
                  onChange={() => {
                    onChange(media.id);
                  }}
                />
                {src === null ? (
                  <span className="attachment-thumb" aria-hidden="true" />
                ) : (
                  <img className="attachment-thumb" src={src} alt="" />
                )}
                <span>{seen(media) ? name : `${name} (${t('mediaRef.private')})`}</span>
              </label>
            );
          })}
        </fieldset>
      )}
      {chosenId === null || browsing ? (
        <LazyItemPicker
          id={`${id}-picker`}
          label={t('mediaRef.pickItem')}
          mode="one"
          picked={item}
          onChange={(next) => {
            setPicked(next);
            setBrowsing(false);
            if (next !== item) {
              onChange(null);
              setAutoChoose(next !== null);
            }
          }}
        />
      ) : (
        <button
          type="button"
          onClick={() => {
            setBrowsing(true);
          }}
        >
          {t('mediaRef.changeItem')}
        </button>
      )}
      <Help id={id} text={help} />
    </fieldset>
  );
};
