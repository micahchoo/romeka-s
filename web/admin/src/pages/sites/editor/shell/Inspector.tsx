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
import { layoutLabel } from '../canvas/summary';
import { Labelled } from '../Labelled';
import { romkaFor } from '../romeka';
import { schemaFor } from '../schema';
import { isJsonArray, isJsonObject, layoutDataObject } from '../values';

const BlockForm = lazy(() =>
  import('../fields').then((module) => ({ default: module.BlockForm })),
);

const RomkaForm = lazy(() =>
  import('../fields').then((module) => ({ default: module.RomkaForm })),
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
      <Labelled id={`block-data-${block.key}`} label={t('editor.unknownData')}>
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
      </Labelled>
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
  const romka = romkaFor(draft.registry, block);
  const layoutData = draft.registry.blockLayoutData;
  const position = draft.blocks.findIndex((entry) => entry.key === block.key) + 1;

  return (
    <div className="block-inspector">
      <h2>
        {t('editor.blockHeading', {
          label: layoutLabel(draft.registry, block, t),
          position,
          count: draft.blocks.length,
        })}
      </h2>
      {romka === undefined ? (
        <Labelled id={`block-layout-${block.key}`} label={t('editor.blockLayout')}>
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
        </Labelled>
      ) : (
        <p className="field-note">
          {t('editor.romkaBlock', { name: t(romka.label) })} {t('editor.romkaGenerated')}
        </p>
      )}
      <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
        {romka === undefined ? (
          schema === undefined ? (
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
          )
        ) : (
          <RomkaForm
            block={block}
            romka={romka}
            issues={issues}
            siteId={siteId}
            dispatch={dispatch}
            options={options}
          />
        )}
        <LayoutDataForm
          idPrefix={`block-${block.key}-layout`}
          owner="block"
          schema={layoutData}
          values={layoutDataObject(block)}
          grid={draft.layout === 'grid'}
          siteId={siteId}
          options={options}
          onChange={(values) => {
            dispatch({ type: 'setLayoutDataValues', blockKey: block.key, values });
          }}
        />
      </Suspense>
    </div>
  );
};
