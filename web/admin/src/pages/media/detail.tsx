import type { ReactElement } from 'react';
import { Suspense, lazy, useState } from 'react';
import { api } from '../../api/client';
import type { Media, MediaId, ThumbnailType } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { formatBytes, formatDateTime } from '../../app/format';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { mediaName, originalFileUrl } from '../../app/media';
import { MediaPreviewState } from '../../app/media-image';
import { useAsync } from '../../app/useAsync';
import { ValuesList } from '../../app/values';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../i18n';
import { MediaEditForm } from './form';
import type { TrackSummary } from './annotations/client';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

/** The stage loads as its own chunk: the list route never needs a player. */
const MediaStage = lazy(() =>
  import('./stage/stage').then((module) => ({ default: module.MediaStage })),
);

/** The notes panel is a second lazy chunk: its list, form and conflict view. */
const MediaNotes = lazy(() =>
  import('./annotations/panel').then((module) => ({ default: module.MediaNotes })),
);

type DerivativeKind = 'original' | ThumbnailType;

const DERIVATIVE_KINDS: readonly DerivativeKind[] = ['original', 'large', 'medium', 'square'];

type ResourcesKey = KeyOf<typeof resources>;

type DetailTranslator = Translator<MessageKey | KeyOf<typeof common> | KeyOf<typeof resources>>;

/** The renderer as words a curator uses, not the module's name. */
const RENDERER_LABELS: Readonly<Record<string, ResourcesKey>> = {
  file: 'media.renderer.file',
  iiif: 'media.renderer.iiif',
  oembed: 'media.renderer.oembed',
  youtube: 'media.renderer.youtube',
  html: 'media.renderer.html',
  iiif_presentation: 'media.renderer.iiifPresentation',
};

/** The ingester as words a curator uses, not the module's name. */
const INGESTER_LABELS: Readonly<Record<string, ResourcesKey>> = {
  upload: 'media.ingester.upload',
  url: 'media.ingester.url',
  html: 'media.ingester.html',
  iiif: 'media.ingester.iiif',
  oembed: 'media.ingester.oembed',
  youtube: 'media.ingester.youtube',
  iiif_presentation: 'media.ingester.iiifPresentation',
  fallback: 'media.ingester.fallback',
};

/** A word from the table, else the stored value itself rather than a blank. */
const plainWord = (
  labels: Readonly<Record<string, ResourcesKey>>,
  value: string,
  translator: DetailTranslator,
): string => {
  const key = labels[value];
  return key === undefined ? value : translator.t(key);
};

const derivativeUrl = (media: Media, kind: DerivativeKind): string | null =>
  kind === 'original' ? originalFileUrl(media) : (media.thumbnailUrls[kind] ?? null);

/**
 * `Admin/MediaController::showAction`: one media, its stage, the metadata the
 * API carries, the file derivatives, and its values.
 */
export const MediaDetail = ({ mediaId }: { readonly mediaId: MediaId }): ReactElement => {
  const translator = useTranslator(common, resources);
  const { t, locale } = translator;
  const state = useAsync(() => api.getMedia(mediaId), [mediaId]);
  const [edited, setEdited] = useState<Media | null>(null);
  // One player for the page: the stage renders it, the notes panel seeks it.
  const [player, setPlayer] = useState<HTMLMediaElement | null>(null);
  // The caption languages, which the notes panel reads and the player offers.
  const [tracks, setTracks] = useState<readonly TrackSummary[]>([]);

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
      <MediaPreviewState media={media} />

      {/* The stage is the media; the notes panel sits beside it from 1100px. */}
      <div className="media-detail-body">
        <Suspense fallback={<p role="status">{t('loading.media')}</p>}>
          <MediaStage media={media} player={setPlayer} tracks={tracks} />
        </Suspense>
        <Suspense fallback={<p role="status">{t('media.loadingNotes')}</p>}>
          <MediaNotes key={media.id} media={media} player={player} onTracks={setTracks} />
        </Suspense>
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
        <dt>{t('media.shownAs')}</dt>
        <dd>{plainWord(RENDERER_LABELS, media.renderer, translator)}</dd>
        <dt>{t('media.addedBy')}</dt>
        <dd>{plainWord(INGESTER_LABELS, media.ingester, translator)}</dd>
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
                {t(`media.derivative.${kind}`)}
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
