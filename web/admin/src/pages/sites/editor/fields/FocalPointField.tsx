/**
 * The `focalPoint` field control: the point of a sibling image that stays in
 * view when the banner crops it, as whole percents across and down.
 *
 * The control shows the image a sibling field holds (the first of the field's
 * `images` that is shown and set: an asset or a media) and puts the point
 * where the curator clicks. The arrow keys move it in steps of five, so the
 * control works without a pointer. "Center" clears both values, which is the
 * browser's own centred crop. A stored media with no image of its own says so
 * rather than framing a broken picture.
 */
import type { KeyboardEvent, MouseEvent, ReactElement } from 'react';
import { api } from '../../../../api/client';
import { getMedia } from '../../../../api/media';
import { assetIdFromParam } from '../../../../api/parse';
import type { AssetId, JsonValue, MediaId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { largeImageUrl } from '../../../../app/media';
import { useTranslator } from '../../../../i18n';
import { editor } from '../../../../i18n/namespaces/editor';
import type { BlockField } from '../../blockSchema';
import { Help, helpId } from '../Labelled';
import { mediaIdOf } from './MediaRefField';
import { isShown } from './visibility';
import './controls.css';

/** The image a focal point is set on. */
export type FocalImage =
  | { readonly kind: 'asset'; readonly id: AssetId }
  | { readonly kind: 'media'; readonly id: MediaId };

/** The first of `keys` whose field is shown and holds an image. */
export const focalImage = (
  keys: readonly string[],
  record: Readonly<Record<string, JsonValue>>,
  siblings: readonly BlockField[],
): FocalImage | null => {
  for (const key of keys) {
    const field = siblings.find((sibling) => sibling.key === key);
    if (field === undefined || !isShown(field, record, siblings)) {
      continue;
    }
    const value = record[key];
    if (field.type === 'assetRef') {
      const id = assetIdFromParam(typeof value === 'number' || typeof value === 'string' ? String(value) : '');
      if (id !== null) {
        return { kind: 'asset', id };
      }
    } else if (field.type === 'mediaRef') {
      const id = mediaIdOf(value);
      if (id !== null) {
        return { kind: 'media', id };
      }
    }
  }
  return null;
};

const STEP = 5;
const CENTER = 50;

const percent = (value: JsonValue | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? Math.min(100, Math.max(0, Math.round(value))) : null;

const clamp = (value: number): number => Math.min(100, Math.max(0, Math.round(value)));

const imageUrl = async (image: FocalImage | null): Promise<string | null> => {
  if (image === null) {
    return null;
  }
  if (image.kind === 'asset') {
    return (await api.getAsset(image.id)).assetUrl;
  }
  return largeImageUrl(await getMedia(api, image.id));
};

export interface FocalPointFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly x: JsonValue | undefined;
  readonly y: JsonValue | undefined;
  readonly image: FocalImage | null;
  /** Writes both percents; `null` for both is the centred crop. */
  readonly onChange: (x: number | null, y: number | null) => void;
}

export const FocalPointField = ({ id, label, help, x, y, image, onChange }: FocalPointFieldProps): ReactElement => {
  const { t } = useTranslator(editor);
  const url = useAsync(() => imageUrl(image), [image?.kind, image?.id]);
  const loaded = url.status === 'ready';
  const src = loaded ? url.data : null;
  const storedX = percent(x);
  const storedY = percent(y);
  const across = storedX ?? CENTER;
  const down = storedY ?? CENTER;

  const place = (event: MouseEvent<HTMLButtonElement>): void => {
    const box = event.currentTarget.getBoundingClientRect();
    if (box.width === 0 || box.height === 0) {
      return;
    }
    onChange(
      clamp(((event.clientX - box.left) / box.width) * 100),
      clamp(((event.clientY - box.top) / box.height) * 100),
    );
  };
  const nudge = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const moves: Readonly<Record<string, readonly [number, number]>> = {
      ArrowLeft: [-STEP, 0],
      ArrowRight: [STEP, 0],
      ArrowUp: [0, -STEP],
      ArrowDown: [0, STEP],
    };
    const move = moves[event.key];
    if (move === undefined) {
      return;
    }
    event.preventDefault();
    onChange(clamp(across + move[0]), clamp(down + move[1]));
  };

  return (
    <fieldset className="block-field focal-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      {image === null || (loaded && src === null) ? (
        <p className="field-note">{image === null ? t('focal.noImage') : t('focal.notAPicture')}</p>
      ) : (
        <>
          <button
            id={id}
            type="button"
            className="focal-frame"
            aria-label={t('focal.frame', { x: across, y: down })}
            onClick={place}
            onKeyDown={nudge}
          >
            {src === null ? null : <img src={src} alt="" draggable={false} />}
            <span
              className="focal-mark"
              aria-hidden="true"
              style={{ left: `${String(across)}%`, top: `${String(down)}%` }}
            />
          </button>
          {storedX === null && storedY === null ? null : (
            <button
              type="button"
              onClick={() => {
                onChange(null, null);
              }}
            >
              {t('focal.center')}
            </button>
          )}
        </>
      )}
      <Help id={id} text={help} />
    </fieldset>
  );
};
