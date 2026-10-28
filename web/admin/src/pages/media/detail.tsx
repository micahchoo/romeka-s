import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useState } from 'react';
import { api } from '../../api/client';
import { getMedia, retryMediaAccess } from '../../api/media';
import type { Media, MediaId, ThumbnailType } from '../../api/types';
import { formatBytes, formatDateTime } from '../../app/format';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { mayEditMedia, mediaName, originalFileUrl } from '../../app/media';
import { MediaPreviewState } from '../../app/media-image';
import { useAsync } from '../../app/useAsync';
import { ValuesList } from '../../app/values';
import { useSession } from '../../app/auth';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../i18n';
import { MediaAccessLine } from './access';
import { MediaEditForm } from './form';
import { TrashOpener } from '../trash/opener';
import type { TrackSummary } from './annotations/client';
import type { StageSlots } from './stage/slots';
import type { NotesFocus } from './annotations/link';
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
 * How often the page reads the media again while a copy is being made. A
 * transcode runs for minutes, and the worker writes its percentage at most
 * every three seconds, so a shorter interval would only repeat the same read.
 * Nothing polls once the copy settles.
 */
const ACCESS_POLL_MS = 5000;

/**
 * `Admin/MediaController::showAction`: one media, its stage, the metadata the
 * API carries, the file derivatives, and its values.
 */
export const MediaDetail = ({
  mediaId,
  focus = null,
}: {
  readonly mediaId: MediaId;
  /** The notes job the URL asked for, applied when the panel is ready. */
  readonly focus?: NotesFocus | null;
}): ReactElement => {
  const translator = useTranslator(common, resources);
  const { t, locale } = translator;
  const user = useSession();
  const state = useAsync(() => getMedia(api, mediaId), [mediaId]);
  const [edited, setEdited] = useState<Media | null>(null);
  // One player for the page: the stage renders it, the notes panel seeks it.
  const [player, setPlayer] = useState<HTMLMediaElement | null>(null);
  // The caption languages, which the notes panel reads and the player offers.
  const [tracks, setTracks] = useState<readonly TrackSummary[]>([]);
  // The places on the stage the notes panel draws in, and whether it draws the
  // picture itself, so the media shows once with its notes on it.
  const [slots, setSlots] = useState<StageSlots | null>(null);
  const [claimed, setClaimed] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);
  const accessState =
    edited?.access?.state ?? (state.status === 'ready' ? state.data.access?.state : null);
  const itemId = state.status === 'ready' ? (state.data.item?.id ?? null) : null;
  // The item's title for the "Part of item" link; until it answers, or if it
  // cannot be read, the link names the item by its id.
  const itemState = useAsync(
    () => (itemId === null ? Promise.resolve(null) : api.getItem(itemId)),
    [itemId],
  );
  const itemTitle = itemState.status === 'ready' ? (itemState.data?.title ?? null) : null;

  // A copy in flight is the one thing this page reads again on its own. The
  // read replaces the document, so a percentage, a settled copy, or a failure
  // reaches the line above the stage without a reload.
  useEffect(() => {
    if (accessState !== 'pending') {
      return;
    }
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async (): Promise<void> => {
      try {
        const next = await getMedia(api, mediaId);
        if (live) {
          setEdited(next);
        }
      } catch {
        // A read that fails leaves the line as it is; the next poll tries again.
      }
      if (live) {
        timer = setTimeout(() => void poll(), ACCESS_POLL_MS);
      }
    };
    timer = setTimeout(() => void poll(), ACCESS_POLL_MS);
    return () => {
      live = false;
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    };
  }, [accessState, mediaId]);

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
      <MediaAccessLine
        media={media}
        canRetry={mayEditMedia(media, user)}
        onRetry={() => {
          setRetrying(true);
          setRetryError(null);
          void retryMediaAccess(api, mediaId)
            .then((next) => {
              setEdited(next);
            })
            .catch((error: unknown) => {
              setRetryError(error instanceof Error ? error.message : String(error));
            })
            .finally(() => {
              setRetrying(false);
            });
        }}
        retrying={retrying}
        retryError={retryError}
      />

      {/* The stage is the media; the notes panel sits beside it from 1100px. */}
      <div className="media-detail-body">
        <Suspense fallback={<p role="status">{t('loading.media')}</p>}>
          <MediaStage
            media={media}
            player={setPlayer}
            tracks={tracks}
            slots={setSlots}
            claimed={claimed}
          />
        </Suspense>
        <Suspense fallback={<p role="status">{t('media.loadingNotes')}</p>}>
          <MediaNotes
            key={media.id}
            media={media}
            player={player}
            focus={focus}
            onTracks={setTracks}
            stage={slots}
            onClaim={setClaimed}
          />
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
            <a href={`#/items/${String(media.item.id)}`}>{itemTitle ?? `#${String(media.item.id)}`}</a>
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

      <TrashOpener
        label={t('media.deleteLabel')}
        target={{ of: 'media', id: media.id }}
        name={mediaName(media) ?? t('common.untitled')}
      />
    </section>
  );
};
