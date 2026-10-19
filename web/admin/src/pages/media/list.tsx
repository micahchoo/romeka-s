import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { Media } from '../../api/types';
import { ListPage } from '../../app/list';
import { MediaImage } from '../../app/media-image';
import { mediaName, previewImageUrl } from '../../app/media';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';

const PER_PAGE = 25;

const itemCell = (media: Media): ReactElement | string =>
  media.item === null ? '' : <a href={`#/items/${String(media.item.id)}`}>{`#${String(media.item.id)}`}</a>;

/**
 * `Admin/MediaController::browseAction`: the media browse table. The first
 * column is the preview, because no media in this collection has a title and
 * the image is what identifies the row.
 */
export const MediaList = (): ReactElement => {
  const { t } = useTranslator(common);
  const resource = t('resource.media');
  return (
    <ListPage<Media>
      resourceLabel={resource}
      emptyMessage={t('list.empty', { resource })}
      load={(page) => api.listMedia({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' })}
      rowKey={(media) => String(media.id)}
      columns={[
        {
          header: t('column.preview'),
          cell: (media) => (
            <MediaImage
              media={media}
              url={previewImageUrl(media)}
              className="media-preview"
              lazy
            />
          ),
        },
        {
          header: t('column.title'),
          cell: (media) => (
            <a href={`#/media/${String(media.id)}`}>{mediaName(media) ?? t('common.untitled')}</a>
          ),
        },
        { header: t('column.item'), cell: itemCell },
        { header: t('column.mediaType'), cell: (media) => media.mediaType ?? '' },
        { header: t('column.source'), cell: (media) => media.source ?? '' },
        { header: t('column.ingester'), cell: (media) => media.ingester },
      ]}
    />
  );
};
