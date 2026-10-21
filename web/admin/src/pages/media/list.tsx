import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Media, ServerMediaKind } from '../../api/types';
import { ListPage } from '../../app/list';
import { MediaImage, MediaKindBadge, MediaPreviewState } from '../../app/media-image';
import { KIND_LABEL, mediaName, previewImageUrl } from '../../app/media';
import type { MediaKind } from '../../app/media';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

const PER_PAGE = 25;
const KIND_PARAM = 'kind';

interface KindFilter {
  readonly kind: MediaKind;
  readonly names: readonly ServerMediaKind[];
}

/**
 * The filter options, one per server kind the list can narrow to. An image is
 * two server names, so the filter sends both. The server has no document kind:
 * it answers `other-file` for a document and for a plain file alike, so the
 * filter names that one word "File" rather than promise a split it cannot make.
 */
const KIND_FILTERS = [
  { kind: 'image', names: ['local-image', 'iiif-image'] },
  { kind: 'audio', names: ['audio'] },
  { kind: 'video', names: ['video'] },
  { kind: 'pdf', names: ['pdf'] },
  { kind: 'model', names: ['model-3d'] },
  { kind: 'captions', names: ['captions'] },
  { kind: 'embed', names: ['embed'] },
  { kind: 'manifest', names: ['iiif-presentation'] },
  { kind: 'other', names: ['other-file'] },
] as const satisfies readonly KindFilter[];

type FilterKind = (typeof KIND_FILTERS)[number]['kind'];

const isFilterKind = (value: string): value is FilterKind =>
  KIND_FILTERS.some((entry) => entry.kind === value);

/** The query after the route's `?`, which the router leaves out of its segments. */
const queryOf = (hash: string): URLSearchParams => new URLSearchParams(hash.split('?')[1] ?? '');

const readKind = (): FilterKind | null => {
  const value = queryOf(window.location.hash).get(KIND_PARAM);
  return value !== null && isFilterKind(value) ? value : null;
};

/** Keep the filter in the URL, so a reload or a shared link opens the same list. */
const writeKind = (kind: FilterKind | null): void => {
  const query = queryOf(window.location.hash);
  if (kind === null) {
    query.delete(KIND_PARAM);
  } else {
    query.set(KIND_PARAM, kind);
  }
  const search = query.toString();
  const hash = `#/media${search === '' ? '' : `?${search}`}`;
  window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}${hash}`);
};

const namesFor = (kind: FilterKind | null): readonly ServerMediaKind[] =>
  kind === null ? [] : (KIND_FILTERS.find((entry) => entry.kind === kind)?.names ?? []);

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
  const names = namesFor(kind);

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
          api.listMedia({
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
