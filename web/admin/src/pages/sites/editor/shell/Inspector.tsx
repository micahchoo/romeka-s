/**
 * The inspector for the selected block: its layout picker, the form of `o:data`
 * fields, the `o:layout_data` styling fields, and a JSON source field for a
 * layout the registry does not name. Both forms are their own lazy chunk, so
 * the canvas, the fields and the shell load separately.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useState } from 'react';
import { parseJsonObjectOrArrayText } from '../../../../api/parse';
import type { SiteId } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import { blockLayoutOptions } from '../../blockRegistry';
import type { DraftAction, BlockDraft, PageDraft, ValidationIssue } from '../draft';
import type { EditorOptions } from '../options';
import { schemaFor } from '../schema';
import { isJsonArray, isJsonObject, layoutDataObject } from '../values';

const BlockForm = lazy(() =>
  import('../fields').then((module) => ({ default: module.BlockForm })),
);

const LayoutDataForm = lazy(() =>
  import('../fields').then((module) => ({ default: module.LayoutDataForm })),
);

export interface InspectorProps {
  readonly draft: PageDraft;
  readonly block: BlockDraft;
  readonly dispatch: (action: DraftAction) => void;
  readonly issues: readonly ValidationIssue[];
  readonly siteId: SiteId;
  readonly options: EditorOptions;
}

/** A block's data as editable JSON, for a layout the registry does not name. */
const UnknownData = ({
  block,
  dispatch,
}: {
  readonly block: BlockDraft;
  readonly dispatch: (action: DraftAction) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const [text, setText] = useState(() => JSON.stringify(block.data, null, 2));
  const [error, setError] = useState<string | null>(null);

  return (
    <div className="block-form">
      <p className="field-note">{t('editor.unknownLayout')}</p>
      <p>
        <label htmlFor={`block-data-${block.key}`}>{t('editor.unknownData')}</label>
        <textarea
          id={`block-data-${block.key}`}
          rows={8}
          value={text}
          onChange={(event) => {
            const next = event.target.value;
            setText(next);
            try {
              const parsed = parseJsonObjectOrArrayText(next);
              if (isJsonArray(parsed) || isJsonObject(parsed)) {
                setError(null);
                dispatch({ type: 'setBlockData', blockKey: block.key, data: parsed });
                return;
              }
              setError(t('editor.unknownInvalid', { message: 'a JSON object or array' }));
            } catch (cause) {
              setError(
                t('editor.unknownInvalid', {
                  message: cause instanceof Error ? cause.message : String(cause),
                }),
              );
            }
          }}
        />
      </p>
      {error === null ? null : (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
};

export const Inspector = ({
  draft,
  block,
  dispatch,
  issues,
  siteId,
  options,
}: InspectorProps): ReactElement => {
  const { t } = useTranslator();
  const schema = schemaFor(draft.registry, block.layout);
  const layoutData = draft.registry.blockLayoutData;
  const position = draft.blocks.findIndex((entry) => entry.key === block.key) + 1;

  return (
    <div className="block-inspector">
      <h2>
        {t('editor.inspector')}
        {position > 0 ? `: ${String(position)}` : ''}
      </h2>
      <p>
        <label htmlFor={`block-layout-${block.key}`}>{t('editor.blockLayout')}</label>
        <select
          id={`block-layout-${block.key}`}
          value={block.layout}
          onChange={(event) => {
            dispatch({ type: 'setLayout', blockKey: block.key, layout: event.target.value });
          }}
        >
          {blockLayoutOptions(block.layout).map((option) => (
            <option key={option.name} value={option.name}>
              {t(option.labelKey)}
            </option>
          ))}
        </select>
      </p>
      <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
        {schema === undefined ? (
          <UnknownData block={block} dispatch={dispatch} />
        ) : (
          <BlockForm
            block={block}
            schema={schema}
            issues={issues}
            siteId={siteId}
            dispatch={dispatch}
            options={options}
          />
        )}
        <LayoutDataForm
          idPrefix={`block-${block.key}-layout`}
          schema={layoutData}
          values={layoutDataObject(block)}
          siteId={siteId}
          options={options}
          onChange={(key, value) => {
            dispatch({ type: 'setLayoutData', blockKey: block.key, key, value });
          }}
        />
      </Suspense>
    </div>
  );
};
