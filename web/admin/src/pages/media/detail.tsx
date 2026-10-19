import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Media, MediaId, ThumbnailType } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { formatBytes, formatDateTime } from '../../app/format';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { MediaImage } from '../../app/media-image';
import { largeImageUrl, mediaName, originalFileUrl } from '../../app/media';
import { useAsync } from '../../app/useAsync';
import { ValuesList } from '../../app/values';
import { useTranslator } from '../../i18n';
import { MediaEditForm } from './form';

type DerivativeKind = 'original' | ThumbnailType;

const DERIVATIVE_KINDS: readonly DerivativeKind[] = ['original', 'large', 'medium', 'square'];

const derivativeUrl = (media: Media, kind: DerivativeKind): string | null =>
  kind === 'original' ? originalFileUrl(media) : (media.thumbnailUrls[kind] ?? null);

/**
 * `Admin/MediaController::showAction`: one media, its image at full width, the
 * metadata the API carries, the file derivatives, and its values.
 */
export const MediaDetail = ({ mediaId }: { readonly mediaId: MediaId }): ReactElement => {
  const translator = useTranslator();
  const { t, locale } = translator;
  const state = useAsync(() => api.getMedia(mediaId), [mediaId]);
  const [edited, setEdited] = useState<Media | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.media')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const media = edited ?? state.data;
  const derivatives = DERIVATIVE_KINDS.flatMap((kind) => {
    const url = derivativeUrl(media, kind);
    return url === null ? [] : [{ kind, url }];
  });

  return (
    <section>
      <h2>{mediaName(media) ?? t('common.untitled')}</h2>

      <div className="media-hero-frame">
        <MediaImage media={media} url={largeImageUrl(media)} className="media-hero" />
      </div>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{media.id}</dd>
        <dt>{t('media.visibility')}</dt>
        <dd>{media.isPublic ? t('common.public') : t('common.private')}</dd>
        <dt>{t('media.partOfItem')}</dt>
        <dd>
          {media.item === null ? (
            t('common.none')
          ) : (
            <a href={`#/items/${String(media.item.id)}`}>{`#${String(media.item.id)}`}</a>
          )}
        </dd>
        <dt>{t('column.created')}</dt>
        <dd>{formatDateTime(media.created, locale)}</dd>
        <dt>{t('column.modified')}</dt>
        <dd>{media.modified === null ? t('common.never') : formatDateTime(media.modified, locale)}</dd>
        <dt>{t('media.mimeType')}</dt>
        <dd>{media.mediaType ?? t('common.none')}</dd>
        <dt>{t('media.renderer')}</dt>
        <dd>{media.renderer}</dd>
        <dt>{t('media.ingester')}</dt>
        <dd>{media.ingester}</dd>
        <dt>{t('media.source')}</dt>
        <dd>{media.source ?? t('common.none')}</dd>
        <dt>{t('media.filename')}</dt>
        <dd>{media.filename ?? t('common.none')}</dd>
        <dt>{t('media.size')}</dt>
        <dd>{media.size === null ? t('common.none') : formatBytes(media.size, translator)}</dd>
        <dt>{t('media.language')}</dt>
        <dd>{media.lang ?? t('common.none')}</dd>
        <dt>{t('media.altText')}</dt>
        <dd>{media.altText ?? t('common.none')}</dd>
        <dt>{t('media.sha256')}</dt>
        <dd>{media.sha256 ?? t('common.none')}</dd>
      </dl>

      <h2>{t('media.fileDerivatives')}</h2>
      {derivatives.length === 0 ? (
        <p>{t('common.none')}</p>
      ) : (
        <ul className="derivative-list">
          {derivatives.map(({ kind, url }) => (
            <li key={kind}>
              <a href={url} target="_blank" rel="noreferrer">
                {kind}
              </a>
            </li>
          ))}
        </ul>
      )}

      <h2>{t('items.valuesHeading')}</h2>
      <ValuesList values={media.values} />

      <EditPanel label={t('media.editLabel')}>
        <MediaEditForm media={media} onSaved={setEdited} />
      </EditPanel>

      <DeleteControl
        label={t('media.deleteLabel')}
        rowName={mediaName(media) ?? t('common.untitled')}
        onConfirm={async () => {
          await api.deleteMedia(media.id);
          window.location.hash = '#/media';
        }}
      />
    </section>
  );
};
