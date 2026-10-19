/**
 * The block form: one control per field of a block's layout schema, over the
 * block's `o:data` and its `o:attachment` rows. Every edit goes out as an
 * action, so the component holds no block state of its own.
 */
import type { ReactElement } from 'react';
import type { SiteId } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import { useEditorText } from '../messages';
import type { BlockLayoutSchema } from '../../blockSchema';
import type { BlockDraft, DraftAction, ValidationIssue } from '../draft';
import type { EditorOptions } from '../options';
import { dataObject } from '../values';
import { AttachmentsField } from './AttachmentsField';
import { FieldControl } from './FieldControl';
import { editor } from '../../../../i18n/namespaces/editor';

export interface BlockFormProps {
  readonly block: BlockDraft;
  /** The layout's schema, or `undefined` for a layout the registry does not name. */
  readonly schema: BlockLayoutSchema | undefined;
  /** Every issue in the draft; the form shows the ones its own block owns. */
  readonly issues: readonly ValidationIssue[];
  readonly siteId: SiteId;
  readonly dispatch: (action: DraftAction) => void;
  /** The editor's shared lists, so the controls make no requests of their own. */
  readonly options?: EditorOptions;
}

export const BlockForm = ({
  block,
  schema,
  issues,
  siteId,
  dispatch,
  options,
}: BlockFormProps): ReactElement => {
  const { t } = useTranslator(editor);
  const e = useEditorText();
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
            label={e(field.label)}
            help={e(field.help)}
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
            record={data}
            siblings={schema.fields}
            onPatch={(values) => {
              for (const [key, value] of Object.entries(values)) {
                dispatch({ type: 'setField', blockKey: block.key, field: key, value });
              }
            }}
            path={field.key}
            issues={mine}
            blockKey={block.key}
            siteId={siteId}
            idPrefix={idPrefix}
            {...(options === undefined ? {} : { options })}
          />
        ),
      )}
    </div>
  );
};
