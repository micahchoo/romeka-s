/**
 * The `mediaRef` field control: one media of the site, chosen in two steps.
 * The curator finds the item with the item picker, then chooses one of its
 * media from a row of thumbnails. The stored value is the media id.
 *
 * The field states what it takes, from the block's schema (`accepts`): an
 * image, or an image this archive can zoom. The item's other media are listed
 * after the matching ones and disabled with the reason, never hidden: a field
 * that says "this item has no image" while the item holds a video reads as a
 * fault. Above the row, one line counts the item's media by kind.
 *
 * A private media, or any media of a private item, is shown but cannot be
 * chosen, because visitors would not see it. A choice that no longer fits, or
 * that became private later, stays stored, and the field says what visitors
 * see instead: the save keeps the choice and leaves the file out of what
 * visitors read, so an unrelated edit still saves. When the picked item has
 * exactly one media the field can show, the control chooses it at once. Once a
 * media is chosen, the item picker folds away behind "Choose another item", so
 * the inspector shows the choice rather than the whole archive.
 */
import type { ReactElement } from 'react';
import { useEffect, useState } from 'react';
import { api } from '../../../../api/client';
import { getMedia, listMediaForItem } from '../../../../api/media';
import { brandId } from '../../../../api/parse';
import type { ItemId, JsonValue, Media, MediaId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { KIND_LABEL, imagePreviewUrl, mediaKind, mediaName } from '../../../../app/media';
import type { MediaKind } from '../../../../app/media';
import { MediaImage, MediaKindBadge } from '../../../../app/media-image';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { editor } from '../../../../i18n/namespaces/editor';
import { resources } from '../../../../i18n/namespaces/resources';
import type { FieldAccepts } from '../../blockSchema';
import { Help, helpId } from '../Labelled';
import { LazyItemPicker } from './LazyItemPicker';
import './controls.css';

/** The media id a stored value names, `null` for none. */
export const mediaIdOf = (value: JsonValue | undefined): MediaId | null => {
  const id = typeof value === 'string' ? Number(value) : value;
  return typeof id === 'number' && Number.isInteger(id) && id > 0 ? brandId<'MediaId'>(id) : null;
};

/**
 * Whether the field shows this media. `imageService` asks what the server's
 * kind knows: a local image has the derivatives the Image API zooms, while an
 * outside IIIF image is not this archive's to serve.
 */
const shows = (media: Media, accepts: FieldAccepts): boolean => {
  if (accepts === 'any') {
    return true;
  }
  if (accepts === 'imageService' && media.kind !== null) {
    return media.kind === 'local-image';
  }
  return mediaKind(media) === 'image';
};

/** Why the field cannot show this media, or `null` when it can. */
const refusalOf = (media: Media, accepts: FieldAccepts): 'notImage' | 'notZoomable' | null => {
  if (shows(media, accepts)) {
    return null;
  }
  return accepts === 'image' ? 'notImage' : 'notZoomable';
};

/** The item's media counted by kind, in the item's own order. */
const countsOf = (media: readonly Media[]): readonly (readonly [MediaKind, number])[] => {
  const counts = new Map<MediaKind, number>();
  for (const entry of media) {
    const kind = mediaKind(entry);
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  return [...counts.entries()];
};

interface Listing {
  readonly item: ItemId | null;
  readonly media: readonly Media[];
  readonly itemPublic: boolean;
}

/** The item's media in its own order, and whether visitors may see the item. */
const listingOf = async (item: ItemId | null): Promise<Listing> => {
  if (item === null) {
    return { item, media: [], itemPublic: true };
  }
  const [media, record] = await Promise.all([
    listMediaForItem(api, item, { perPage: 200 }).then((page) => page.items),
    api.getItem(item),
  ]);
  return { item, media, itemPublic: record.isPublic };
};

export interface MediaRefFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  /** What the field shows, from the block's schema. */
  readonly accepts: FieldAccepts;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
}

export const MediaRefField = ({
  id,
  label,
  help,
  accepts,
  value,
  onChange,
}: MediaRefFieldProps): ReactElement => {
  const { t, number } = useTranslator(common, editor, resources);
  const chosenId = mediaIdOf(value);
  const chosen = useAsync(
    (): Promise<Media | null> => (chosenId === null ? Promise.resolve(null) : getMedia(api, chosenId)),
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
  const listed = current?.media ?? null;
  const media = listed ?? [];
  const seen = (entry: Media): boolean => entry.isPublic && current?.itemPublic !== false;
  const choosable = (entry: Media): boolean => seen(entry) && shows(entry, accepts);
  const choices = media.filter(choosable);
  const only = choices.length === 1 ? choices[0] : undefined;
  // What the stored choice shows instead of a picture, when it no longer fits.
  const misfit = chosenMedia !== null && !shows(chosenMedia, accepts) ? chosenMedia : null;

  useEffect(() => {
    if (autoChoose && listed !== null) {
      setAutoChoose(false);
      if (only !== undefined) {
        onChange(only.id);
      }
    }
  }, [autoChoose, listed, only, onChange]);

  const thumbnail = chosenMedia === null ? null : imagePreviewUrl(chosenMedia);
  const chosenHidden = chosenMedia !== null && !seen(chosenMedia);
  return (
    <fieldset className="block-field media-ref-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      {chosenId === null ? null : (
        <p className="attachment-item">
          {chosenMedia === null ? (
            <span className="attachment-thumb" aria-hidden="true" />
          ) : (
            <MediaImage
              media={chosenMedia}
              url={thumbnail}
              className="attachment-thumb"
              compact
              decorative
            />
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
      {misfit === null ? null : (
        <p className="field-note">
          {t(accepts === 'imageService' ? 'mediaRef.misfitZoom' : 'mediaRef.misfit', {
            kind: t(KIND_LABEL[mediaKind(misfit)]),
          })}
        </p>
      )}
      {item === null || listed === null ? null : media.length === 0 ? (
        <p className="field-note">{t('mediaRef.noImages')}</p>
      ) : (
        <>
          <p className="field-note media-ref-facts">
            {t('mediaRef.mediaInItem')}
            {countsOf(media).map(([kind, count]) => (
              <span className="media-kind-badge" key={kind}>
                {number(count)} {t(KIND_LABEL[kind])}
              </span>
            ))}
          </p>
          <fieldset className="media-choice">
            <legend>{t('mediaRef.choose')}</legend>
            {media.map((entry) => {
              const name = mediaName(entry) ?? t('common.untitled');
              const refusal = refusalOf(entry, accepts);
              const reasons = [
                seen(entry) ? null : t('mediaRef.private'),
                refusal === null
                  ? null
                  : t(refusal === 'notImage' ? 'mediaRef.notImage' : 'mediaRef.notZoomable'),
              ].filter((reason) => reason !== null);
              return (
                <label key={String(entry.id)} aria-disabled={!choosable(entry)}>
                  <input
                    type="radio"
                    name={`${id}-media`}
                    checked={chosenId === entry.id}
                    disabled={!choosable(entry)}
                    onChange={() => {
                      onChange(entry.id);
                    }}
                  />
                  <MediaImage
                    media={entry}
                    url={imagePreviewUrl(entry)}
                    className="attachment-thumb"
                    compact
                    decorative
                  />
                  <span>{name}</span>
                  {mediaKind(entry) === 'image' ? null : <MediaKindBadge media={entry} />}
                  {reasons.length === 0 ? null : (
                    <span className="media-ref-reason">({reasons.join(', ')})</span>
                  )}
                </label>
              );
            })}
          </fieldset>
        </>
      )}
      {chosenId === null || browsing ? (
        <LazyItemPicker
          id={`${id}-picker`}
          label={t('mediaRef.pickItem')}
          mode="one"
          accepts={accepts}
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
