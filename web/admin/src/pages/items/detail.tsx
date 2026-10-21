import type { ReactElement } from 'react';
import { Suspense, lazy, useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { api } from '../../api/client';
import { listMediaForItem } from '../../api/media';
import { currentUser, subscribeSession } from '../../api/session';
import type { ClassId, Item, ItemId, Media, MediaId, User, UserId } from '../../api/types';
import { fromBrowserEntry, fromDrop } from '../../app/upload/pick';
import type { Picked } from '../../app/upload/pick';
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
const OUTLINE_MS = 600;

/**
 * Loaded the first time the panel opens, so an item page that nobody adds to
 * pays none of the panel's code.
 */
const AddMediaPanel = lazy(() => import('./add-media/panel'));

/** The roles whose create the server accepts (`write/access.rs`), before the item check. */
const ADDING_ROLES: readonly string[] = ['author', 'reviewer', 'editor', 'site_admin', 'global_admin'];

/**
 * `write/media.rs`'s `item()`: a media create is allowed when the caller may
 * update the item. An unknown session shows the button, because the server
 * decides and the key may carry the right; a known role answers here.
 */
const mayAddMedia = (item: Item, user: User | null): boolean => {
  if (user === null) {
    return true;
  }
  if (!ADDING_ROLES.includes(user.role)) {
    return false;
  }
  return user.role !== 'author' || item.owner?.id === user.id;
};

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

const isFileDrag = (event: DragEvent): boolean =>
  event.dataTransfer !== null && Array.from(event.dataTransfer.types).includes('Files');

/** One media of the item: the image links to the original file. */
const MediaCard = ({ media, outlined }: { readonly media: Media; readonly outlined: boolean }): ReactElement => {
  const { t } = useTranslator(common, resources);
  const href = previewLinkUrl(media);
  const url = previewImageUrl(media);
  const image = <MediaImage media={media} url={url} className="media-card-image" lazy />;
  return (
    <figure className={outlined ? 'media-card media-card-new' : 'media-card'}>
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

/** Files dragged over the whole window: the media section outlines and takes the drop. */
const useWindowFiles = (
  enabled: boolean,
  onDropFiles: (picked: Picked) => void,
): { readonly count: number | null } => {
  const [count, setCount] = useState<number | null>(null);
  const depth = useRef(0);

  useEffect(() => {
    if (!enabled) {
      return;
    }
    const enter = (event: DragEvent): void => {
      if (!isFileDrag(event)) {
        return;
      }
      event.preventDefault();
      depth.current += 1;
      const transfer = event.dataTransfer;
      setCount(transfer === null ? 1 : Array.from(transfer.items).filter((item) => item.kind === 'file').length);
    };
    const over = (event: DragEvent): void => {
      if (isFileDrag(event)) {
        event.preventDefault();
      }
    };
    const leave = (): void => {
      depth.current -= 1;
      if (depth.current <= 0) {
        depth.current = 0;
        setCount(null);
      }
    };
    const drop = (event: DragEvent): void => {
      if (!isFileDrag(event)) {
        return;
      }
      event.preventDefault();
      depth.current = 0;
      setCount(null);
      // The items are readable only inside this handler.
      const transfer = event.dataTransfer;
      const entries = Array.from(transfer?.items ?? [], (item) => (item.kind === 'file' ? fromBrowserEntry(item.webkitGetAsEntry()) : null));
      const files = Array.from(transfer?.files ?? []);
      void fromDrop(entries, files).then(onDropFiles);
    };
    window.addEventListener('dragenter', enter);
    window.addEventListener('dragover', over);
    window.addEventListener('dragleave', leave);
    window.addEventListener('drop', drop);
    return () => {
      window.removeEventListener('dragenter', enter);
      window.removeEventListener('dragover', over);
      window.removeEventListener('dragleave', leave);
      window.removeEventListener('drop', drop);
    };
  }, [enabled, onDropFiles]);

  return { count };
};

export const ItemDetail = ({ itemId }: { readonly itemId: ItemId }): ReactElement => {
  const { t, locale } = useTranslator(common, resources);
  const text = useTermLabels();
  const user = useSyncExternalStore(subscribeSession, currentUser);
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
      listMediaForItem(api, itemId, { perPage: MEDIA_PER_PAGE }),
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
  const [panelOpen, setPanelOpen] = useState(false);
  const [added, setAdded] = useState<readonly Media[]>([]);
  const [outlined, setOutlined] = useState<MediaId | null>(null);
  const [incoming, setIncoming] = useState<Picked | null>(null);
  const onDropFiles = useCallback((picked: Picked) => {
    setIncoming(picked);
    setPanelOpen(true);
  }, []);
  const { count: dropCount } = useWindowFiles(readsState.status === 'ready', onDropFiles);

  useEffect(() => {
    if (outlined === null) {
      return;
    }
    const timer = setTimeout(() => {
      setOutlined(null);
    }, OUTLINE_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [outlined]);

  const onAdded = useCallback((media: Media) => {
    setAdded((current) => (current.some((entry) => entry.id === media.id) ? current : [...current, media]));
    setOutlined(media.id);
  }, []);

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
  const adding = mayAddMedia(item, user);
  const media = reads === null ? [] : [...reads.media, ...added.filter((entry) => !reads.media.some((known) => known.id === entry.id))];
  const mediaTotal = reads === null ? 0 : reads.mediaTotal + Math.max(0, media.length - reads.media.length);
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
        <div className={dropCount === null ? 'item-media' : 'item-media item-media-drop'}>
          <div className="item-media-head">
            <h2>{t('items.mediaHeading', { count: mediaTotal })}</h2>
            {adding ? (
              <button
                type="button"
                aria-expanded={panelOpen}
                onClick={() => {
                  setPanelOpen(!panelOpen);
                }}
              >
                {panelOpen ? t('items.closeAddMedia') : t('items.addMedia')}
              </button>
            ) : (
              <p className="item-media-note">{t('items.mayNotAdd')}</p>
            )}
          </div>

          {dropCount === null ? null : (
            <p className="item-media-drop-note" role="status">
              {dropCount === 1 ? t('items.dropToAddOne') : t('items.dropToAdd', { count: dropCount })}
            </p>
          )}

          {panelOpen && adding ? (
            <Suspense fallback={<p role="status">{t('list.loading', { resource: t('resource.media') })}</p>}>
              <AddMediaPanel
                item={item}
                onAdded={onAdded}
                onClose={() => {
                  setPanelOpen(false);
                }}
                incoming={incoming}
                onIncoming={() => {
                  setIncoming(null);
                }}
              />
            </Suspense>
          ) : null}

          {media.length === 0 ? (
            <p className="item-media-empty">{t('items.noMediaYet')}</p>
          ) : (
            <ul className="media-grid">
              {media.map((entry) => (
                <li key={entry.id}>
                  <MediaCard media={entry} outlined={outlined === entry.id} />
                </li>
              ))}
            </ul>
          )}
        </div>
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
