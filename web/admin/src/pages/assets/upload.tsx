/**
 * The one upload control for a site asset. It is hosted above the asset table
 * and inside every asset field, so the pick, the drop, the refusal before send
 * and the description a content slot asks for behave the same in both places.
 *
 * The upload is the existing multipart `POST /api/assets` the client builds;
 * this control owns the file input, the drag state and the description step,
 * and tells its host which asset now exists. A decorative slot asks for no
 * description, because a background is not read by anyone.
 */
import type { DragEvent, ReactElement } from 'react';
import { useEffect, useRef, useState } from 'react';
import { api } from '../../api/client';
import type { Asset } from '../../api/types';
import { SaveStatus } from '../../app/fields';
import { formatError } from '../../app/format';
import { assetKind, extensionOfName, fileImageUrl } from '../../app/media';
import { KindTile } from '../../app/media-image';
import { useSave } from '../../app/save';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { admin } from '../../i18n/namespaces/admin';

export interface AssetTileProps {
  readonly asset: Asset;
  /** Size class, shared by a picture and the kind's placeholder. */
  readonly className: string;
  readonly compact?: boolean;
  /** Empty `alt`: the host draws the name beside the tile. */
  readonly decorative?: boolean;
}

/** One asset, as a picture when a browser draws it and the kind's tile otherwise. */
export const AssetTile = ({
  asset,
  className,
  compact = false,
  decorative = true,
}: AssetTileProps): ReactElement => (
  <KindTile
    kind={assetKind(asset)}
    url={fileImageUrl(asset.mediaType, asset.assetUrl)}
    name={asset.name === '' ? asset.filename : asset.name}
    extension={extensionOfName(asset.filename)}
    className={className}
    compact={compact}
    decorative={decorative}
  />
);

type UploadState =
  | { readonly phase: 'idle' }
  | { readonly phase: 'sending'; readonly name: string }
  | { readonly phase: 'done'; readonly asset: Asset }
  | { readonly phase: 'failed'; readonly message: string };

export interface AssetUploadProps {
  /** The host's id, which prefixes the file input and the description field. */
  readonly id: string;
  /** The field's accept attribute, `null` when the slot takes any file. */
  readonly accept: string | null;
  /** Whether the host shows a description. A background slot does not. */
  readonly describe: boolean;
  /** The asset that exists, on upload and again once its description saves. */
  readonly onAsset: (asset: Asset) => void;
}

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

/** Whether a file the browser names can meet this field's accept set. */
const meetsAccept = (file: File, accept: string | null): boolean =>
  accept === null || file.type === '' || file.type.startsWith('image/');

export const AssetUpload = ({
  id,
  accept,
  describe,
  onAsset,
}: AssetUploadProps): ReactElement => {
  const translator = useTranslator(admin);
  const { t } = translator;
  const [state, setState] = useState<UploadState>({ phase: 'idle' });
  const [description, setDescription] = useState('');
  const [dragging, setDragging] = useState(false);
  const descriptionBox = useRef<HTMLTextAreaElement>(null);
  const { state: saveState, save } = useSave();

  const upload = (file: File): void => {
    if (!meetsAccept(file, accept)) {
      setState({
        phase: 'failed',
        message: t('assets.refusedNotImage', { name: file.name }),
      });
      return;
    }
    setState({ phase: 'sending', name: file.name });
    setDescription('');
    api.createAsset({ name: file.name, altText: null }, file).then(
      (asset) => {
        setState({ phase: 'done', asset });
        setDescription(asset.altText ?? '');
        onAsset(asset);
      },
      (cause: unknown) => {
        setState({
          phase: 'failed',
          message: t('assets.uploadError', {
            message: formatError(toError(cause), translator),
          }),
        });
      },
    );
  };

  const doneId = state.phase === 'done' ? state.asset.id : null;
  // The description is the reason the upload happened: put the caret there the
  // first time the asset exists, and not again after every save.
  useEffect(() => {
    if (doneId !== null && describe) {
      descriptionBox.current?.focus();
    }
  }, [doneId, describe]);

  const saveDescription = (): void => {
    if (state.phase !== 'done') {
      return;
    }
    const asset = state.asset;
    save(async () => {
      const updated = await api.updateAsset(asset.id, {
        name: asset.name,
        altText: nullable(description),
      });
      setState({ phase: 'done', asset: updated });
      onAsset(updated);
    });
  };

  const dropped = (event: DragEvent<HTMLDivElement>): void => {
    event.preventDefault();
    setDragging(false);
    const file = event.dataTransfer.files[0];
    if (file !== undefined) {
      upload(file);
    }
  };

  const current = state.phase === 'done' ? state.asset : null;
  const kind = current === null ? null : assetKind(current);

  return (
    <div
      className={`asset-upload${dragging ? ' is-dragging' : ''}`}
      onDragOver={(event) => {
        if (event.dataTransfer.types.includes('Files')) {
          event.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={() => {
        setDragging(false);
      }}
      onDrop={dropped}
    >
      <p className="asset-upload-row">
        <label className="asset-upload-trigger" htmlFor={`${id}-upload`}>
          {t('assets.upload')}
        </label>
        <input
          id={`${id}-upload`}
          type="file"
          accept={accept ?? undefined}
          disabled={state.phase === 'sending'}
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = '';
            if (file !== undefined) {
              upload(file);
            }
          }}
        />
        <span className="setting-help">{t('assets.uploadHint')}</span>
      </p>

      {state.phase === 'sending' ? (
        <p className="asset-busy" role="status">
          {t('assets.uploading', { name: state.name })}
        </p>
      ) : null}

      {state.phase === 'failed' ? (
        <p className="field-error" role="alert">
          {state.message}
        </p>
      ) : null}

      {state.phase === 'done' ? (
        <p className="asset-busy" role="status">
          {t('assets.uploaded', { name: state.asset.name })}
        </p>
      ) : null}

      {current !== null && describe ? (
        <div className="asset-describe">
          <label htmlFor={`${id}-description`}>
            {kind === 'image' ? t('assets.describeImage') : t('assets.linkText')}
          </label>
          {kind === 'image' ? null : (
            <p className="setting-help">{t('assets.linkTextHelp')}</p>
          )}
          <textarea
            id={`${id}-description`}
            ref={descriptionBox}
            rows={2}
            value={description}
            onChange={(event) => {
              setDescription(event.target.value);
            }}
          />
          <p>
            <button
              type="button"
              disabled={saveState.status === 'saving'}
              onClick={saveDescription}
            >
              {t('assets.saveDescription')}
            </button>
          </p>
          <SaveStatus state={saveState} />
        </div>
      ) : null}
    </div>
  );
};
