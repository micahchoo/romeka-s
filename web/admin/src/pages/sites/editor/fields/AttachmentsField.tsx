/**
 * The `attachments` field control: the block's `o:attachment` rows, each an item
 * with an optional media and an HTML caption. The rows live outside `o:data`
 * (`site_page_block.attachment`), so a change goes through `setAttachments`
 * rather than `setField`.
 *
 * The item picker searches `/api/items`; the media select is limited to the
 * chosen item's media through `api.listMediaForItem`, which the server reads as
 * `item_id` (`MediaAdapter::buildQuery`).
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../../../api/client';
import type { Item, ItemId, MediaId } from '../../../../api/types';
import { formatError } from '../../../../app/format';
import { toError, useAsync } from '../../../../app/useAsync';
import { mediaName } from '../../../../app/media';
import { useTranslator } from '../../../../i18n';
import type { BlockAttachment } from '../draft';
import { HtmlEditor } from './HtmlEditor';

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

const ItemPicker = ({
  id,
  label,
  value,
  onChange,
}: {
  readonly id: string;
  readonly label: string;
  readonly value: ItemId | null;
  readonly onChange: (value: ItemId | null) => void;
}): ReactElement => {
  const translator = useTranslator();
  const { t } = translator;
  const [term, setTerm] = useState('');
  const [results, setResults] = useState<readonly Item[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const selected = useAsync(
    (): Promise<Item | null> => (value === null ? Promise.resolve(null) : api.getItem(value)),
    [value],
  );

  const search = (): void => {
    setError(null);
    api.listItems({ search: term, perPage: 10 }).then(
      (page) => {
        setResults(page.items);
      },
      (cause: unknown) => {
        setError(formatError(toError(cause), translator));
      },
    );
  };

  return (
    <div className="block-field attachment-item">
      <label htmlFor={`${id}-search`}>{label}</label>
      <p className="attachment-current">
        {value === null
          ? t('attachments.none')
          : selected.status === 'ready' && selected.data !== null
            ? (selected.data.title ?? t('common.untitled'))
            : selected.status === 'error'
              ? t('attachments.itemFallback', { id: value })
              : t('attachments.itemLoading')}
      </p>
      <p>
        <input
          id={`${id}-search`}
          type="search"
          value={term}
          onChange={(event) => {
            setTerm(event.target.value);
          }}
        />
        <button type="button" onClick={search}>
          {t('query.search')}
        </button>
        {value === null ? null : (
          <button
            type="button"
            onClick={() => {
              onChange(null);
              setResults(null);
            }}
          >
            {t('attachments.change')}
          </button>
        )}
      </p>
      {error === null ? null : (
        <p className="field-error" role="alert">
          {t('attachments.searchError', { message: error })}
        </p>
      )}
      {results === null ? null : results.length === 0 ? (
        <p className="attachment-empty">{t('attachments.noResults')}</p>
      ) : (
        <ul className="item-results">
          {results.map((item) => (
            <li key={String(item.id)}>
              <button
                type="button"
                onClick={() => {
                  onChange(item.id);
                  setResults(null);
                }}
              >
                {item.title ?? t('common.untitled')}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

/** One media option of the media select. */
interface MediaOption {
  readonly id: MediaId;
  readonly label: string;
}

const MediaSelect = ({
  id,
  itemId,
  value,
  onChange,
}: {
  readonly id: string;
  readonly itemId: ItemId | null;
  readonly value: MediaId | null;
  readonly onChange: (value: MediaId | null) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(
    (): Promise<readonly MediaOption[]> =>
      itemId === null
        ? Promise.resolve([])
        : api.listMediaForItem(itemId, { perPage: 200 }).then((page) =>
            page.items.map((media) => ({
              id: media.id,
              label: mediaName(media) ?? t('common.untitled'),
            })),
          ),
    [itemId, t],
  );
  const options = state.status === 'ready' ? state.data : [];
  return (
    <div className="block-field">
      <label htmlFor={id}>{t('attachments.media')}</label>
      <select
        id={id}
        value={value === null ? '' : String(value)}
        disabled={itemId === null}
        onChange={(event) => {
          const media = options.find((option) => String(option.id) === event.target.value);
          onChange(media?.id ?? null);
        }}
      >
        <option value="">{t('attachments.noMedia')}</option>
        {options.map((option) => (
          <option key={String(option.id)} value={String(option.id)}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
};

export interface AttachmentsFieldProps {
  readonly id: string;
  readonly label: string;
  /** Whether an attachment may name a media (`blockAttachmentsForm`). */
  readonly media: boolean;
  readonly attachments: readonly BlockAttachment[];
  readonly onChange: (attachments: readonly BlockAttachment[]) => void;
}

export const AttachmentsField = ({
  id,
  label,
  media,
  attachments,
  onChange,
}: AttachmentsFieldProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <fieldset className="block-field attachments-field">
      <legend>{label}</legend>
      <ul className="attachment-rows">
        {attachments.map((attachment, index) => (
          <li className="attachment-row" key={String(index)}>
            <ItemPicker
              id={`${id}-${String(index)}-item`}
              label={t('attachments.item')}
              value={attachment.item}
              onChange={(item) => {
                onChange(replaceAt(attachments, index, { ...attachment, item, media: null }));
              }}
            />
            {media ? (
              <MediaSelect
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
      <p>
        <button
          type="button"
          onClick={() => {
            onChange([...attachments, { item: null, media: null, caption: '' }]);
          }}
        >
          {t('attachments.add')}
        </button>
      </p>
    </fieldset>
  );
};
