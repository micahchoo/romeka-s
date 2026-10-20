/**
 * The controls of the field kinds the content blocks bring: one item
 * (`itemRef`) and a list of items each with a media and a caption
 * (`itemMedia`). The list is the attachments control kept in the block's
 * Romeka settings, because an `html` carrier holds no attachment rows.
 */
import type { ReactElement } from 'react';
import { brandId } from '../../../../api/parse';
import type { ItemId, JsonValue } from '../../../../api/types';
import type { BlockAttachment } from '../draft';
import { useEditorText } from '../messages';
import { isJsonObject } from '../values';
import { AttachmentsField } from './AttachmentsField';
import { WithHelp } from './controlBase';
import type { ControlProps, FieldOf } from './controlBase';
import { LazyItemPicker } from './LazyItemPicker';

/** A stored id, from a number or a numeric string, `null` for anything else. */
const idOf = (value: JsonValue | undefined): number | null => {
  const id = typeof value === 'string' ? Number(value) : value;
  return typeof id === 'number' && Number.isInteger(id) && id > 0 ? id : null;
};

/** The stored records as attachment rows, skipping a record with no item. */
export const asItemMedia = (value: JsonValue | undefined): readonly BlockAttachment[] =>
  Array.isArray(value)
    ? value.flatMap((record: JsonValue): readonly BlockAttachment[] => {
        if (!isJsonObject(record)) {
          return [];
        }
        const item = idOf(record.item);
        const media = idOf(record.media);
        return item === null
          ? []
          : [
              {
                item: brandId<'ItemId'>(item),
                media: media === null ? null : brandId<'MediaId'>(media),
                caption: typeof record.caption === 'string' ? record.caption : '',
              },
            ];
      })
    : [];

/** The rows as the records the block stores. */
export const toItemMedia = (rows: readonly BlockAttachment[]): JsonValue =>
  rows.flatMap((row): JsonValue[] =>
    row.item === null ? [] : [{ item: row.item, media: row.media, caption: row.caption }],
  );

export const ItemRefControl = ({
  field,
  id,
  value,
  onChange,
  error,
}: ControlProps & { readonly field: FieldOf<'itemRef'> }): ReactElement => {
  const e = useEditorText();
  const picked = idOf(value);
  return (
    <WithHelp id={id} help={e(field.help)}>
      <LazyItemPicker
        id={id}
        label={e(field.label)}
        mode="one"
        picked={picked === null ? null : brandId<'ItemId'>(picked)}
        onChange={(next: ItemId | null) => {
          onChange(next);
        }}
      />
      {error}
    </WithHelp>
  );
};

export const ItemMediaControl = ({
  field,
  id,
  value,
  onChange,
}: ControlProps & { readonly field: FieldOf<'itemMedia'> }): ReactElement => {
  const e = useEditorText();
  return (
    <AttachmentsField
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      media
      attachments={asItemMedia(value)}
      onChange={(rows) => {
        onChange(toItemMedia(rows));
      }}
    />
  );
};

/** The props of a control of one of this module's kinds. */
export type ContentControlProps = ControlProps & { readonly field: FieldOf<'itemRef' | 'itemMedia'> };

/** The control of an `itemRef` or `itemMedia` field. */
export const ContentControl = (props: ContentControlProps): ReactElement =>
  props.field.type === 'itemRef' ? (
    <ItemRefControl {...props} field={props.field} />
  ) : (
    <ItemMediaControl {...props} field={props.field} />
  );
