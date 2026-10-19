/**
 * The `attachments` field control: the block's `o:attachment` rows, each an item
 * with an optional media and an HTML caption. The rows live outside `o:data`
 * (`site_page_block.attachment`), so a change goes through `setAttachments`
 * rather than `setField`.
 *
 * Items are picked with the shared `ItemPicker` (search as you type,
 * thumbnails, keyboard): picking an item appends its row and unpicking removes
 * it. Each row shows the item's thumbnail and title. The media choice is a row
 * of thumbnails limited to the item's media through `api.listMediaForItem`,
 * which the server reads as `item_id` (`MediaAdapter::buildQuery`).
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { Item, ItemId, Media, MediaId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { mediaName, previewImageUrl } from '../../../../app/media';
import { useTranslator } from '../../../../i18n';
import type { BlockAttachment } from '../draft';
import { Help, helpId } from '../Labelled';
import { useEditorText } from '../messages';
import { HtmlEditor } from './HtmlEditor';
import { ItemPicker } from './ItemPicker';
import './controls.css';

/** Replaces one entry of a list, the one place a row edit writes the list. */
const replaceAt = <T,>(values: readonly T[], index: number, next: T): readonly T[] =>
  values.map((value, at) => (at === index ? next : value));

/** Moves one entry by `delta` places, or returns the list unchanged at an edge. */
const moveAt = <T,>(values: readonly T[], index: number, delta: number): readonly T[] => {
  const target = index + delta;
  const moved = values[index];
  if (moved === undefined || target < 0 || target >= values.length) {
    return values;
  }
  const next = [...values];
  next.splice(index, 1);
  next.splice(target, 0, moved);
  return next;
};

/** The rows after the picker's set changed: unpicked items leave, new ones append. */
export const syncAttachments = (
  attachments: readonly BlockAttachment[],
  picked: readonly ItemId[],
): readonly BlockAttachment[] => {
  const kept = attachments.filter((row) => row.item === null || picked.includes(row.item));
  const added = picked
    .filter((item) => !kept.some((row) => row.item === item))
    .map((item): BlockAttachment => ({ item, media: null, caption: '' }));
  return [...kept, ...added];
};

/** The picked item's thumbnail and title at the top of its row. */
const AttachedItem = ({ itemId }: { readonly itemId: ItemId | null }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(
    (): Promise<Item | null> => (itemId === null ? Promise.resolve(null) : api.getItem(itemId)),
    [itemId],
  );
  const item = state.status === 'ready' ? state.data : null;
  const thumbnail = item?.thumbnailDisplayUrls.square;
  return (
    <p className="attachment-item">
      {thumbnail === undefined ? (
        <span className="attachment-thumb" aria-hidden="true" />
      ) : (
        <img className="attachment-thumb" src={thumbnail} alt="" />
      )}
      <strong>
        {itemId === null
          ? t('attachments.none')
          : item !== null
            ? (item.title ?? t('common.untitled'))
            : state.status === 'error'
              ? t('attachments.itemFallback', { id: itemId })
              : t('attachments.itemLoading')}
      </strong>
    </p>
  );
};

const mediaThumbnail = (media: Media): string | null =>
  media.thumbnailDisplayUrls.square ?? previewImageUrl(media);

/** The media an attachment shows: the item's main media, or one of its media. */
const MediaChoice = ({
  id,
  itemId,
  value,
  onChange,
}: {
  readonly id: string;
  readonly itemId: ItemId | null;
  readonly value: MediaId | null;
  readonly onChange: (value: MediaId | null) => void;
}): ReactElement | null => {
  const { t } = useTranslator();
  const e = useEditorText();
  const state = useAsync(
    (): Promise<readonly Media[]> =>
      itemId === null
        ? Promise.resolve([])
        : api.listMediaForItem(itemId, { perPage: 200 }).then((page) => page.items),
    [itemId],
  );
  const media = state.status === 'ready' ? state.data : [];
  if (itemId === null) {
    return null;
  }
  return (
    <fieldset className="media-choice">
      <legend>{t('attachments.media')}</legend>
      <label>
        <input
          type="radio"
          name={id}
          checked={value === null}
          onChange={() => {
            onChange(null);
          }}
        />
        <span className="attachment-thumb" aria-hidden="true" />
        <span>{e('attachments.mainMedia')}</span>
      </label>
      {media.map((entry) => {
        const thumbnail = mediaThumbnail(entry);
        return (
          <label key={String(entry.id)}>
            <input
              type="radio"
              name={id}
              checked={value === entry.id}
              onChange={() => {
                onChange(entry.id);
              }}
            />
            {thumbnail === null ? (
              <span className="attachment-thumb" aria-hidden="true" />
            ) : (
              <img className="attachment-thumb" src={thumbnail} alt="" />
            )}
            <span>{mediaName(entry) ?? t('common.untitled')}</span>
          </label>
        );
      })}
    </fieldset>
  );
};

export interface AttachmentsFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  /** Whether an attachment may name a media (`blockAttachmentsForm`). */
  readonly media: boolean;
  readonly attachments: readonly BlockAttachment[];
  readonly onChange: (attachments: readonly BlockAttachment[]) => void;
}

export const AttachmentsField = ({
  id,
  label,
  help,
  media,
  attachments,
  onChange,
}: AttachmentsFieldProps): ReactElement => {
  const { t } = useTranslator();
  const e = useEditorText();
  const picked = [
    ...new Set(attachments.flatMap((row) => (row.item === null ? [] : [row.item]))),
  ];
  return (
    <fieldset className="block-field attachments-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      <Help id={id} text={help} />
      {attachments.length === 0 ? null : (
        <ul className="attachment-rows" aria-label={label}>
          {attachments.map((attachment, index) => (
            <li className="attachment-row" key={String(index)}>
              <AttachedItem itemId={attachment.item} />
              {media ? (
                <MediaChoice
                  id={`${id}-${String(index)}-media`}
                  itemId={attachment.item}
                  value={attachment.media}
                  onChange={(next) => {
                    onChange(replaceAt(attachments, index, { ...attachment, media: next }));
                  }}
                />
              ) : null}
              <HtmlEditor
                id={`${id}-${String(index)}-caption`}
                label={t('attachments.caption')}
                value={attachment.caption}
                onChange={(caption) => {
                  onChange(replaceAt(attachments, index, { ...attachment, caption }));
                }}
              />
              <p className="attachment-actions">
                <button
                  type="button"
                  disabled={index === 0}
                  aria-label={t('attachments.moveUp', { position: index + 1 })}
                  onClick={() => {
                    onChange(moveAt(attachments, index, -1));
                  }}
                >
                  {t('blocks.moveUpText')}
                </button>
                <button
                  type="button"
                  disabled={index === attachments.length - 1}
                  aria-label={t('attachments.moveDown', { position: index + 1 })}
                  onClick={() => {
                    onChange(moveAt(attachments, index, 1));
                  }}
                >
                  {t('blocks.moveDownText')}
                </button>
                <button
                  type="button"
                  aria-label={t('attachments.remove', { position: index + 1 })}
                  onClick={() => {
                    onChange(attachments.filter((_, at) => at !== index));
                  }}
                >
                  {t('action.remove')}
                </button>
              </p>
            </li>
          ))}
        </ul>
      )}
      <ItemPicker
        id={`${id}-picker`}
        label={e('attachments.pick')}
        picked={picked}
        onChange={(next) => {
          onChange(syncAttachments(attachments, next));
        }}
      />
    </fieldset>
  );
};
