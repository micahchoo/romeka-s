import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ClassId, Item, ItemId, Media, UserId } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { formatDateTime } from '../../app/format';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { MediaImage, MediaKindBadge } from '../../app/media-image';
import { mediaName, previewImageUrl, previewLinkUrl } from '../../app/media';
import { useAsync } from '../../app/useAsync';
import { ValuesList } from '../../app/values';
import { writtenItems } from '../../app/written';
import { useTranslator } from '../../i18n';
import { useTermLabels } from '../../i18n/termText';
import { classLabelOf, classLabels, ownerNameOf, userNames } from './labels';
import type { TermName } from './labels';
import { ItemEditForm } from './form';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

const MEDIA_PER_PAGE = 100;

/** The reads the record needs beside it: its media and the labels of its refs. */
interface ItemReads {
  readonly media: readonly Media[];
  readonly mediaTotal: number;
  readonly ownerNames: ReadonlyMap<UserId, string>;
  readonly classLabels: ReadonlyMap<ClassId, TermName>;
}

/** The label maps before their reads answer, so a ref falls back to `#id`. */
const NO_NAMES: ReadonlyMap<UserId, string> = new Map();
const NO_CLASSES: ReadonlyMap<ClassId, TermName> = new Map();

/** One media of the item: the image links to the original file. */
const MediaCard = ({ media }: { readonly media: Media }): ReactElement => {
  const { t } = useTranslator(common, resources);
  const href = previewLinkUrl(media);
  const url = previewImageUrl(media);
  const image = <MediaImage media={media} url={url} className="media-card-image" lazy />;
  return (
    <figure className="media-card">
      {href === null || url === null ? (
        image
      ) : (
        <a href={href} target="_blank" rel="noreferrer">
          {image}
        </a>
      )}
      <figcaption className="media-card-caption">
        {mediaName(media) ?? t('common.untitled')}
        <span className="media-card-meta">
          <MediaKindBadge media={media} />
          {media.mediaType === null ? null : <span>{media.mediaType}</span>}
        </span>
      </figcaption>
    </figure>
  );
};

export const ItemDetail = ({ itemId }: { readonly itemId: ItemId }): ReactElement => {
  const { t, locale } = useTranslator(common, resources);
  const text = useTermLabels();
  // A create hands its record to this page, so the heading and the fields
  // render without reading the item back.
  const [seeded] = useState(() => writtenItems.take(itemId));
  const itemState = useAsync(
    () => (seeded === null ? api.getItem(itemId) : Promise.resolve(seeded)),
    [itemId],
  );
  // The media and the label lookups are separate reads: the record renders as
  // soon as it is known and these fill in when they answer.
  const readsState = useAsync(async (): Promise<ItemReads> => {
    const [media, users, classes] = await Promise.all([
      api.listMediaForItem(itemId, { perPage: MEDIA_PER_PAGE }),
      api.listUsers({ perPage: 100 }),
      api.listResourceClasses({ perPage: 100 }),
    ]);
    return {
      media: media.items,
      mediaTotal: media.total,
      ownerNames: userNames(users.items),
      classLabels: classLabels(classes.items),
    };
  }, [itemId]);
  // A save replaces the read view without a refetch, so the editor and the
  // record above it agree until the next navigation.
  const [edited, setEdited] = useState<Item | null>(null);

  if (itemState.status === 'loading') {
    return <p role="status">{t('list.loading', { resource: t('resource.items') })}</p>;
  }
  if (itemState.status === 'error') {
    return <ErrorNotice error={itemState.error} />;
  }

  const reads = readsState.status === 'ready' ? readsState.data : null;
  const item = edited ?? itemState.data;
  const ownerName = ownerNameOf(reads?.ownerNames ?? NO_NAMES, item.owner);
  const classLabel = classLabelOf(reads?.classLabels ?? NO_CLASSES, item.resourceClass, text);
  return (
    <section>
      <h2>{item.title ?? t('common.untitled')}</h2>
      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{item.id}</dd>
        <dt>{t('media.visibility')}</dt>
        <dd>{item.isPublic ? t('common.public') : t('common.private')}</dd>
        <dt>{t('column.class')}</dt>
        <dd>{classLabel ?? t('common.none')}</dd>
        <dt>{t('field.resourceTemplate')}</dt>
        <dd>{item.resourceTemplate === null ? t('common.none') : `#${String(item.resourceTemplate.id)}`}</dd>
        <dt>{t('column.owner')}</dt>
        <dd>{ownerName ?? t('common.noOwner')}</dd>
        <dt>{t('column.created')}</dt>
        <dd>{formatDateTime(item.created, locale)}</dd>
        <dt>{t('column.modified')}</dt>
        <dd>{item.modified === null ? t('common.never') : formatDateTime(item.modified, locale)}</dd>
      </dl>

      {readsState.status === 'error' ? (
        <ErrorNotice error={readsState.error} />
      ) : reads === null ? (
        <p role="status">{t('list.loading', { resource: t('resource.media') })}</p>
      ) : (
        <>
          <h2>{t('items.mediaHeading', { count: reads.mediaTotal })}</h2>
          {reads.media.length === 0 ? (
            <p>{t('items.noMedia')}</p>
          ) : (
            <ul className="media-grid">
              {reads.media.map((entry) => (
                <li key={entry.id}>
                  <MediaCard media={entry} />
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      <h2>{t('items.valuesHeading')}</h2>
      <ValuesList values={item.values} />

      <EditPanel label={t('items.editLabel')}>
        <ItemEditForm item={item} onSaved={setEdited} />
      </EditPanel>

      <DeleteControl
        label={t('items.deleteLabel')}
        rowName={item.title ?? t('items.rowName', { id: item.id })}
        onConfirm={async () => {
          await api.deleteItem(item.id);
          window.location.hash = '#/items';
        }}
      />
    </section>
  );
};
