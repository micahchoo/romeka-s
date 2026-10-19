/**
 * The Romeka settings form: one control per field of a Romeka block's schema,
 * over the carrier's `o:layout_data.romeka.settings`. It reuses the same
 * `FieldControl` table as an upstream block form, so a Romeka block offers the
 * same controls.
 */
import type { ReactElement } from 'react';
import type { SiteId } from '../../../../api/types';
import type { RomkaBlockSchema } from '../../blockSchema';
import type { BlockDraft, DraftAction, ValidationIssue } from '../draft';
import type { EditorOptions } from '../options';
import { settingsOf } from '../romeka';
import { FieldControl } from './FieldControl';

export interface RomkaFormProps {
  readonly block: BlockDraft;
  /** The Romeka block the carrier names. */
  readonly romka: RomkaBlockSchema;
  /** Every issue in the draft; the form shows the ones its own block owns. */
  readonly issues: readonly ValidationIssue[];
  readonly siteId: SiteId;
  readonly dispatch: (action: DraftAction) => void;
  /** The editor's shared lists, so the controls make no requests of their own. */
  readonly options?: EditorOptions;
}

export const RomkaForm = ({
  block,
  romka,
  issues,
  siteId,
  dispatch,
  options,
}: RomkaFormProps): ReactElement => {
  const settings = settingsOf(block);
  const mine = issues.filter((issue) => issue.blockKey === block.key);
  const idPrefix = `block-${block.key}`;
  return (
    <div className="block-form">
      {romka.fields.map((field) => (
        <FieldControl
          key={field.key}
          field={field}
          value={settings[field.key]}
          onChange={(value) => {
            dispatch({
              type: 'setRomkaSetting',
              blockKey: block.key,
              field: field.key,
              value,
            });
          }}
          record={settings}
          siblings={romka.fields}
          onPatch={(values) => {
            for (const [key, value] of Object.entries(values)) {
              dispatch({ type: 'setRomkaSetting', blockKey: block.key, field: key, value });
            }
          }}
          path={field.key}
          issues={mine}
          blockKey={block.key}
          siteId={siteId}
          idPrefix={idPrefix}
          {...(options === undefined ? {} : { options })}
        />
      ))}
    </div>
  );
};
