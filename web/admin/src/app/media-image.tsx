import type { ReactElement, ReactNode } from 'react';
import type { Media } from '../api/types';
import { useTranslator } from '../i18n';
import { KIND_LABEL, fileExtension, mediaKind, mediaName } from './media';
import type { MediaKind } from './media';
import { resources } from '../i18n/namespaces/resources';

/** One line drawing per kind, stroked round so it reads at 1.75rem. */
const ICONS: Readonly<Record<MediaKind, ReactNode>> = {
  image: (
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <circle cx="8.5" cy="10" r="1.5" />
      <path d="m4 17 5-5 4 4 3-3 4 4" />
    </>
  ),
  audio: (
    <>
      <path d="M9 18V6l10-2v12" />
      <circle cx="6.5" cy="18" r="2.5" />
      <circle cx="16.5" cy="16" r="2.5" />
    </>
  ),
  video: (
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="m10 9 5 3-5 3z" />
    </>
  ),
  pdf: (
    <>
      <path d="M6 3h8l4 4v14H6z" />
      <path d="M14 3v4h4" />
      <path d="M12 11v6m0 0-2-2m2 2 2-2" />
    </>
  ),
  document: (
    <>
      <path d="M6 3h8l4 4v14H6z" />
      <path d="M14 3v4h4" />
      <path d="M9 12h6M9 15h6M9 18h4" />
    </>
  ),
  model: (
    <>
      <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9z" />
      <path d="m4 7.5 8 4.5 8-4.5M12 12v9" />
    </>
  ),
  captions: (
    <>
      <rect x="3" y="6" width="18" height="12" rx="2" />
      <path d="M10 10.5a3 3 0 1 0 0 3M17 10.5a2.5 2.5 0 1 0 0 3" />
    </>
  ),
  embed: <path d="m9 8-4 4 4 4M15 8l4 4-4 4" />,
  manifest: (
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="M7 9h4M7 12h4M7 15h4M14 9h3M14 12h3M14 15h3" />
    </>
  ),
  other: (
    <>
      <path d="M6 3h8l4 4v14H6z" />
      <path d="M14 3v4h4" />
    </>
  ),
};

export const MediaKindIcon = ({ kind }: { readonly kind: MediaKind }): ReactElement => (
  <svg
    className="media-kind-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.6}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    {ICONS[kind]}
  </svg>
);

/**
 * The words a preview needs while it is not ready: `Making preview` for a queued
 * or running derivative, and the tool's reason when it failed. A ready preview,
 * a media an older server sent no state for, and a kind with no derivative say
 * nothing, because their tile already reads as it did before this state existed.
 */
const usePreviewText = (media: Media): string | null => {
  const { t } = useTranslator(resources);
  const preview = media.preview;
  if (preview === null) {
    return null;
  }
  if (preview.state === 'pending') {
    return t('media.preview.pending');
  }
  if (preview.state === 'failed') {
    return preview.reason === null
      ? t('media.preview.failed')
      : t('media.preview.failedWithReason', { reason: preview.reason });
  }
  return null;
};

/** The preview's state as one line, for a list cell or a page heading. */
export const MediaPreviewState = ({
  media,
  className = '',
}: {
  readonly media: Media;
  readonly className?: string;
}): ReactElement | null => {
  const text = usePreviewText(media);
  if (text === null) {
    return null;
  }
  const failed = media.preview?.state === 'failed';
  return (
    <span className={`media-preview-state${failed ? ' is-failed' : ''} ${className}`.trim()}>
      {text}
    </span>
  );
};

/** A media's kind as a word, for a tile that is not only a picture. */
export const MediaKindBadge = ({ media }: { readonly media: Media }): ReactElement => {
  const { t } = useTranslator(resources);
  return <span className="media-kind-badge">{t(KIND_LABEL[mediaKind(media)])}</span>;
};

export interface MediaImageProps {
  readonly media: Media;
  /** Image URL. `null` renders the kind's placeholder, so a file is never an `<img>`. */
  readonly url: string | null;
  /** Size class, applied to the image and to the placeholder. */
  readonly className: string;
  readonly lazy?: boolean;
  /**
   * Icon only, for a box too small for words (the browse table's 3rem cell).
   * The words stay in the DOM for a screen reader and in the title for a pointer.
   */
  readonly compact?: boolean;
  /** A preview beside its own name: empty `alt`, and nothing for a screen reader. */
  readonly decorative?: boolean;
}

export interface KindTileProps {
  readonly kind: MediaKind;
  /** Image URL. `null` renders the kind's placeholder, so a file is never an `<img>`. */
  readonly url: string | null;
  /** The name a screen reader reads when the tile shows a picture. */
  readonly name: string;
  /** The file's extension, upper case, when its name carries one. */
  readonly extension: string | null;
  /** Size class, applied to the image and to the placeholder. */
  readonly className: string;
  readonly lazy?: boolean;
  /**
   * Icon only, for a box too small for words (the browse table's 3rem cell).
   * The words stay in the DOM for a screen reader and in the title for a pointer.
   */
  readonly compact?: boolean;
  /** A preview beside its own name: empty `alt`, and nothing for a screen reader. */
  readonly decorative?: boolean;
  /** What a derivative that is not ready says; `null` for a tile with no state. */
  readonly preview?: string | null;
}

/**
 * One preview, in the same box whether it is an image or a placeholder: a
 * picture, or the kind in words and the file extension. A media and an asset
 * both draw through this, so an asset of any kind never lands in an `<img>`
 * that cannot show it.
 */
export const KindTile = ({
  kind,
  url,
  name,
  extension,
  className,
  lazy = false,
  compact = false,
  decorative = false,
  preview = null,
}: KindTileProps): ReactElement => {
  const { t } = useTranslator(resources);
  if (url === null) {
    const label = t(KIND_LABEL[kind]);
    // A PDF file's extension repeats the kind's own name, so it earns no line.
    const namedFile = extension !== null && extension.toLowerCase() !== label.toLowerCase();
    const words = namedFile ? `${label} ${extension}` : label;
    const spoken = preview === null ? words : `${words}. ${preview}`;
    return (
      <span
        className={`media-image media-image-empty ${className}${compact ? ' is-compact' : ''}`}
        data-kind={kind}
        title={compact ? spoken : undefined}
        aria-hidden={decorative ? true : undefined}
      >
        <MediaKindIcon kind={kind} />
        <span className="media-image-kind">{label}</span>
        {namedFile ? <span className="media-image-extension">{extension}</span> : null}
        {!compact && preview !== null ? (
          <span className="media-preview-state media-image-state">{preview}</span>
        ) : null}
      </span>
    );
  }
  const picture = (
    <img
      className={`media-image ${className}`}
      src={url}
      alt={decorative ? '' : name}
      loading={lazy ? 'lazy' : 'eager'}
    />
  );
  if (kind === 'image') {
    return picture;
  }
  const label = t(KIND_LABEL[kind]);
  // A poster or a PDF page is a picture of the media, not the media: the badge
  // keeps it from reading as an image.
  return (
    <span
      className={`media-image-frame${compact ? ' is-compact' : ''}`}
      data-kind={kind}
    >
      {picture}
      <span className="media-kind-badge media-image-badge" title={label}>
        <MediaKindIcon kind={kind} />
        <span className="media-image-badge-word">{label}</span>
      </span>
    </span>
  );
};

/**
 * One media preview, in the same box whether it is an image or a placeholder.
 * A placeholder names the kind and the file extension, because a curator
 * meeting an audio file in a grid still needs to know what it is.
 */
export const MediaImage = ({
  media,
  url,
  className,
  lazy = false,
  compact = false,
  decorative = false,
}: MediaImageProps): ReactElement => {
  const { t } = useTranslator(resources);
  const preview = usePreviewText(media);
  return (
    <KindTile
      kind={mediaKind(media)}
      url={url}
      name={mediaName(media) ?? t('media.altFallback', { id: media.id })}
      extension={fileExtension(media)}
      className={className}
      lazy={lazy}
      compact={compact}
      decorative={decorative}
      preview={preview}
    />
  );
};
