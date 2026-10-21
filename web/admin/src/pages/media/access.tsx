/**
 * The access copy's state on the media page and the media list.
 *
 * A media whose original no browser can play gets a copy the worker makes; the
 * page names that copy by what it is ("Making a playable copy, 40%") rather
 * than by the tool. A failed copy offers Retry to a reader who may edit the
 * media, and a copy this install cannot make says which tool is missing and
 * offers no button, because pressing it would change nothing.
 */
import type { ReactElement } from 'react';
import type { Media } from '../../api/types';
import { fileExtension, KIND_LABEL, mediaKind } from '../../app/media';
import { formatBytes } from '../../app/format';
import type { KeyOf, MessageKey, Translator } from '../../i18n';

type AccessTranslator = Translator<MessageKey | KeyOf<typeof resources>>;
import { useTranslator } from '../../i18n';
import { resources } from '../../i18n/namespaces/resources';

type AccessKey = KeyOf<typeof resources>;

/** What the copy is, in the words a curator uses. */
type CopyTarget = 'playable' | 'image' | 'pdf';

const TARGET_LABEL: Readonly<Record<CopyTarget, AccessKey>> = {
  playable: 'media.access.makingPlayable',
  image: 'media.access.makingImage',
  pdf: 'media.access.makingPdf',
};

/** The copy a media's kind gets, as the server's access table decides it. */
const copyTarget = (media: Media): CopyTarget => {
  const kind = mediaKind(media);
  if (kind === 'audio' || kind === 'video') {
    return 'playable';
  }
  return kind === 'document' ? 'pdf' : 'image';
};

/** A copy whose tool this install lacks: the server's words name the tool. */
export const needsTool = (media: Media): boolean =>
  media.access?.state === 'none' && (media.access.reason?.startsWith('needs ') ?? false);

/**
 * Whether the stage should state the original beside the copy: an access row
 * that plans, made, or failed to make a copy. A row that says the original is
 * what the reader gets has nothing to contrast.
 */
export const showsOriginal = (media: Media): boolean =>
  media.access !== null &&
  media.originalUrl !== null &&
  (media.access.state !== 'none' || needsTool(media));

/** The original's format and size, as "AVI, 467 MB". */
export const originalFacts = (media: Media, translator: AccessTranslator): string => {
  const format = fileExtension(media) ?? translator.t(KIND_LABEL[mediaKind(media)]);
  return media.size === null ? format : `${format}, ${formatBytes(media.size, translator)}`;
};

/** One line for a copy in flight, failed, or impossible on this install. */
export const MediaAccessLine = ({
  media,
  canRetry,
  onRetry,
  retrying = false,
  retryError = null,
}: {
  readonly media: Media;
  /** Whether this reader may edit the media, so a Retry button would work. */
  readonly canRetry: boolean;
  readonly onRetry?: (() => void) | undefined;
  readonly retrying?: boolean;
  readonly retryError?: string | null;
}): ReactElement | null => {
  const { t } = useTranslator(resources);
  const access = media.access;
  if (access === null || access.state === 'ready') {
    return null;
  }
  if (access.state === 'none' && !needsTool(media)) {
    return null;
  }
  if (access.state === 'none') {
    return (
      <p className="media-access-state">
        {t('media.access.noCopy', { reason: access.reason ?? '' })}
      </p>
    );
  }
  if (access.state === 'pending') {
    const target = copyTarget(media);
    const text =
      target === 'playable' && access.progress !== null
        ? t('media.access.makingPlayablePercent', { percent: access.progress })
        : t(TARGET_LABEL[target]);
    return (
      <p className="media-access-state" role="status">
        {text}
      </p>
    );
  }
  return (
    <div className="media-access-state is-failed">
      <p role="status">
        {access.reason === null
          ? t('media.access.failed')
          : t('media.access.failedWithReason', { reason: access.reason })}
      </p>
      {canRetry && onRetry !== undefined ? (
        <button
          type="button"
          className="button-link"
          disabled={retrying}
          onClick={onRetry}
        >
          {retrying ? t('media.access.retrying') : t('media.access.retry')}
        </button>
      ) : null}
      {retryError === null ? null : (
        <p role="alert">{t('media.access.retryFailed', { message: retryError })}</p>
      )}
    </div>
  );
};

/** The compact cue a media list shows while a copy is being made. */
export const MediaAccessCue = ({ media }: { readonly media: Media }): ReactElement | null => {
  const { t } = useTranslator(resources);
  if (media.access?.state !== 'pending') {
    return null;
  }
  return (
    <span className="media-preview-state">{t(TARGET_LABEL[copyTarget(media)])}</span>
  );
};
