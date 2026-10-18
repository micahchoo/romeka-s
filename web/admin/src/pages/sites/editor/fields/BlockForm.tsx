/**
 * The block form: one control per field of a block's layout schema, over the
 * block's `o:data` and its `o:attachment` rows. Every edit goes out as an
 * action, so the component holds no block state of its own.
 */
import type { ReactElement } from 'react';
import type { SiteId } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import type { BlockLayoutSchema } from '../../blockSchema';
import type { BlockDraft, DraftAction, ValidationIssue } from '../draft';
import { dataObject } from '../values';
import { AttachmentsField } from './AttachmentsField';
import { FieldControl } from './FieldControl';

export interface BlockFormProps {
  readonly block: BlockDraft;
  /** The layout's schema, or `undefined` for a layout the registry does not name. */
  readonly schema: BlockLayoutSchema | undefined;
  /** Every issue in the draft; the form shows the ones its own block owns. */
  readonly issues: readonly ValidationIssue[];
  readonly siteId: SiteId;
  readonly dispatch: (action: DraftAction) => void;
}

export const BlockForm = ({
  block,
  schema,
  issues,
  siteId,
  dispatch,
}: BlockFormProps): ReactElement => {
  const { t } = useTranslator();
  if (schema === undefined) {
    return <p className="field-note">{t('field.unknownLayout')}</p>;
  }
  const data = dataObject(block);
  const mine = issues.filter((issue) => issue.blockKey === block.key);
  const idPrefix = `block-${block.key}`;
  return (
    <div className="block-form">
      {schema.fields.map((field) =>
        field.type === 'attachments' ? (
          <AttachmentsField
            key={field.key}
            id={`${idPrefix}-${field.key}`}
            label={t(field.label)}
            media={field.media}
            attachments={block.attachments}
            onChange={(attachments) => {
              dispatch({ type: 'setAttachments', blockKey: block.key, attachments });
            }}
          />
        ) : (
          <FieldControl
            key={field.key}
            field={field}
            value={data[field.key]}
            onChange={(value) => {
              dispatch({ type: 'setField', blockKey: block.key, field: field.key, value });
            }}
            path={field.key}
            issues={mine}
            blockKey={block.key}
            siteId={siteId}
            idPrefix={idPrefix}
          />
        ),
      )}
    </div>
  );
};
