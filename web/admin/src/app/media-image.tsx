import type { ReactElement } from 'react';
import type { Media } from '../api/types';
import { useTranslator } from '../i18n';
import { mediaName } from './media';
import { resources } from '../i18n/namespaces/resources';

export interface MediaImageProps {
  readonly media: Media;
  /** Image URL. `null` renders the placeholder, so a missing file never breaks. */
  readonly url: string | null;
  /** Size class, applied to the image and to the placeholder. */
  readonly className: string;
  readonly lazy?: boolean;
}

/**
 * One media preview. A row with no thumbnail and no original shows a labelled
 * placeholder instead of an image with a dead `src`.
 */
export const MediaImage = ({
  media,
  url,
  className,
  lazy = false,
}: MediaImageProps): ReactElement => {
  const { t } = useTranslator(resources);
  if (url === null) {
    return <span className={`media-image media-image-empty ${className}`}>{t('media.noPreview')}</span>;
  }
  return (
    <img
      className={`media-image ${className}`}
      src={url}
      alt={mediaName(media) ?? t('media.altFallback', { id: media.id })}
      loading={lazy ? 'lazy' : 'eager'}
    />
  );
};
