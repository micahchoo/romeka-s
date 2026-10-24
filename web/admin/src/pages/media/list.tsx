import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { listMedia } from '../../api/media';
import type { Media } from '../../api/types';
import { ListPage } from '../../app/list';
import { MediaImage, MediaKindBadge, MediaPreviewState } from '../../app/media-image';
import { KIND_FILTERS, KIND_LABEL, isFilterKind, mediaName, previewImageUrl, serverKindsOf } from '../../app/media';
import type { FilterKind } from '../../app/media';
import { hashQuery, replaceHashQuery } from '../../app/router';
import { MediaAccessCue } from './access';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

const PER_PAGE = 25;
const KIND_PARAM = 'kind';

const readKind = (): FilterKind | null => {
  const value = hashQuery(window.location.hash).get(KIND_PARAM);
  return value !== null && isFilterKind(value) ? value : null;
};

/** Keep the filter in the URL, so a reload or a shared link opens the same list. */
const writeKind = (kind: FilterKind | null): void => {
  const query = hashQuery(window.location.hash);
  if (kind === null) {
    query.delete(KIND_PARAM);
  } else {
    query.set(KIND_PARAM, kind);
  }
  replaceHashQuery('media', query);
};

const itemCell = (media: Media): ReactElement | string =>
  media.item === null ? '' : <a href={`#/items/${String(media.item.id)}`}>{`#${String(media.item.id)}`}</a>;

/**
 * `Admin/MediaController::browseAction`: the media browse table. The first
 * column is the preview, because no media in this collection has a title and
 * the image is what identifies the row.
 */
export const MediaList = (): ReactElement => {
  const { t } = useTranslator(common, resources);
  const resource = t('resource.media');
  const [kind, setKind] = useState<FilterKind | null>(readKind);
  const names = kind === null ? [] : serverKindsOf(kind);

  const choose = (value: string): void => {
    const next = isFilterKind(value) ? value : null;
    writeKind(next);
    setKind(next);
  };

  return (
    <section>
      <p>
        {t('media.browse.intro')} <a href="#/items">{t('media.browse.items')}</a>.{' '}
        {t('media.browse.batch')} <a href="#/import">{t('media.browse.import')}</a>.
      </p>
      <p>
        <label htmlFor="media-kind-filter">{t('media.filter.label')}</label>{' '}
        <select
          id="media-kind-filter"
          value={kind ?? ''}
          onChange={(event) => {
            choose(event.target.value);
          }}
        >
          <option value="">{t('media.filter.all')}</option>
          {KIND_FILTERS.map((entry) => (
            <option key={entry.kind} value={entry.kind}>
              {t(KIND_LABEL[entry.kind])}
            </option>
          ))}
        </select>
      </p>
      <ListPage<Media>
        key={kind ?? 'all'}
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={(page) =>
          listMedia(api, {
            page,
            perPage: PER_PAGE,
            sortBy: 'id',
            sortOrder: 'asc',
            ...(names.length === 0 ? {} : { romekaKind: names }),
          })
        }
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
                compact
              />
            ),
          },
          {
            header: t('column.title'),
            cell: (media) => (
              <span className="media-title-cell">
                <a href={`#/media/${String(media.id)}`}>{mediaName(media) ?? t('common.untitled')}</a>
                <MediaKindBadge media={media} />
                <MediaPreviewState media={media} />
                <MediaAccessCue media={media} />
              </span>
            ),
          },
          { header: t('column.item'), cell: itemCell },
          { header: t('column.mediaType'), cell: (media) => media.mediaType ?? '' },
          { header: t('column.source'), cell: (media) => media.source ?? '' },
          { header: t('column.ingester'), cell: (media) => media.ingester },
        ]}
      />
    </section>
  );
};
