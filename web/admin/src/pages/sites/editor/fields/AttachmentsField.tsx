/**
 * The `attachments` field control: the block's `o:attachment` rows, each an item
 * with an optional media and an HTML caption. The rows live outside `o:data`
 * (`site_page_block.attachment`), so a change goes through `setAttachments`
 * rather than `setField`.
 *
 * Items are picked with the shared item picker (`LazyItemPicker`): picking an
 * item appends its row, unpicking removes it, and the tray's order is the rows'
 * order. Each row shows the item's thumbnail and title. The media choice is a row
 * of thumbnails limited to the item's media through `listMediaForItem`,
 * which the server reads as `item_id` (`MediaAdapter::buildQuery`).
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import { listMediaForItem } from '../../../../api/media';
import type { Item, ItemId, Media, MediaId } from '../../../../api/types';
import {
  ReorderButtons,
  ReorderHandle,
  ReorderStatus,
  useReorderable,
} from '../../../../app/reorder/useReorderable';
import { useAsync } from '../../../../app/useAsync';
import { mediaName, previewImageUrl } from '../../../../app/media';
import { MediaKindBadge } from '../../../../app/media-image';
import { useTranslator } from '../../../../i18n';
import type { BlockAttachment } from '../draft';
import { Help, helpId } from '../Labelled';
import { useEditorText } from '../messages';
import { HtmlEditor } from './HtmlEditor';
import { LazyItemPicker } from './LazyItemPicker';
import { common } from '../../../../i18n/namespaces/common';
import { editor } from '../../../../i18n/namespaces/editor';
import './controls.css';

/** Replaces one entry of a list, the one place a row edit writes the list. */
const replaceAt = <T,>(values: readonly T[], index: number, next: T): readonly T[] =>
  values.map((value, at) => (at === index ? next : value));

/**
 * The rows after the picker's picks changed. The rows follow the picks' order,
 * which is the tray's: unpicked items leave, a new pick gets an empty row, and
 * a row with no item stays at the end.
 */
export const syncAttachments = (
  attachments: readonly BlockAttachment[],
  picked: readonly ItemId[],
): readonly BlockAttachment[] => [
  ...picked.flatMap((item): readonly BlockAttachment[] => {
    const rows = attachments.filter((row) => row.item === item);
    return rows.length > 0 ? rows : [{ item, media: null, caption: '' }];
  }),
  ...attachments.filter((row) => row.item === null),
];

/** The picked item's thumbnail and title at the top of its row. */
const AttachedItem = ({ itemId }: { readonly itemId: ItemId | null }): ReactElement => {
  const { t } = useTranslator(common, editor);
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
  const { t } = useTranslator(common, editor);
  const e = useEditorText();
  const state = useAsync(
    (): Promise<readonly Media[]> =>
      itemId === null
        ? Promise.resolve([])
        : listMediaForItem(api, itemId, { perPage: 200 }).then((page) => page.items),
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
            <MediaKindBadge media={entry} />
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
  const { t } = useTranslator(common, editor);
  const e = useEditorText();
  const reorder = useReorderable({
    items: attachments,
    onReorder: onChange,
    labelOf: (_, index) => t('attachments.row', { position: index + 1 }),
  });
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
            <li className="attachment-row" key={String(index)} {...reorder.row(index)}>
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
                <ReorderHandle reorder={reorder} index={index} />
                <ReorderButtons reorder={reorder} index={index} />
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
      <ReorderStatus reorder={reorder} />
      <LazyItemPicker
        id={`${id}-picker`}
        label={e('attachments.pick')}
        mode="many"
        accepts="any"
        picked={picked}
        onChange={(next) => {
          onChange(syncAttachments(attachments, next));
        }}
      />
    </fieldset>
  );
};
