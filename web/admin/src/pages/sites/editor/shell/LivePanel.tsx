/**
 * The page editor's preview: the device bar, the status line and the live
 * surface (`../frame/LiveFrame.tsx`). It is its own lazy chunk, fetched on the
 * editor's first render beside the Outline.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import type { JsonValue } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import { useInsertChoices } from '../canvas/BlockPicker';
import { DeviceBar } from '../DeviceBar';
import type { BlockKey, DraftAction, PageDraft } from '../draft';
import { LiveFrame } from '../frame/LiveFrame';
import type { PickerChoice } from '../frame/LiveFrame';
import type { InsertTarget } from '../frame/geometry';
import type { DeviceWidth } from '../frameFit';
import type { Selection } from './selection';
import type { PreviewState } from './usePreview';
import { sites } from '../../../../i18n/namespaces/sites';

export interface LivePanelProps {
  readonly preview: PreviewState;
  readonly draft: PageDraft;
  readonly selection: Selection | null;
  readonly siteSettings: JsonValue | null;
  readonly onSelect: (key: BlockKey) => void;
  readonly dispatch: (action: DraftAction) => void;
}

const insertAction = (target: InsertTarget, choice: PickerChoice): DraftAction =>
  choice.kind === 'template'
    ? { type: 'insertBlocks', index: target.index, blocks: choice.template.blocks }
    : {
        type: 'addBlock',
        layout: choice.layout,
        index: target.index,
        ...(target.kind === 'group' ? { into: target.group } : {}),
      };

export const LivePanel = ({
  preview,
  draft,
  selection,
  siteSettings,
  onSelect,
  dispatch,
}: LivePanelProps): ReactElement => {
  const { t } = useTranslator(sites);
  const [device, setDevice] = useState<DeviceWidth>('fit');
  const templates = useInsertChoices(siteSettings);

  return (
    <section className="editor-preview" role="region" aria-label={t('editor.previewLabel')}>
      <div className="editor-preview-bar">
        <DeviceBar device={device} onChange={setDevice} />
        <span className="editor-preview-status" role="status">
          {preview.status === 'loading' ? t('editor.previewLoading') : ''}
        </span>
      </div>
      {preview.render === null ? (
        <div className="editor-preview-frame">
          <p className="editor-preview-empty">{t('editor.previewLabel')}</p>
        </div>
      ) : (
        <LiveFrame
          render={preview.render}
          device={device}
          draft={draft}
          selection={selection}
          templates={templates}
          onSelect={onSelect}
          onInsert={(target, choice) => {
            dispatch(insertAction(target, choice));
          }}
        />
      )}
      {preview.status === 'error' && preview.message !== null ? (
        <p className="field-error" role="alert">
          {t('editor.previewError', { message: preview.message })}
        </p>
      ) : null}
    </section>
  );
};
