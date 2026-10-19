/**
 * The inspector when no block is selected: the page's own members. The title,
 * slug and visibility edit the page row; the layout picker and the grid fields
 * edit the page's `o:layout_data`, which the renderer reads for every page.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { PageId } from '../../../../api/types';
import { DeleteControl } from '../../../../app/delete';
import { useTranslator } from '../../../../i18n';
import { useEditorText } from '../messages';
import { gridGap, pageColumns } from '../canvas/grid';
import type { DraftAction, PageDraft, ValidationIssue } from '../draft';
import { Checked, Labelled } from '../Labelled';

export interface PageSettingsProps {
  readonly draft: PageDraft;
  readonly dispatch: (action: DraftAction) => void;
  readonly issues: readonly ValidationIssue[];
  readonly pageId: PageId | null;
  /** Called after the page row is deleted, so the shell can return to the site. */
  readonly onDeleted: () => void;
  /** Open the shell's save-as-template dialog. */
  readonly onOpenSaveTemplate: () => void;
}

const numberFrom = (text: string): number => {
  const value = Number.parseInt(text, 10);
  return Number.isNaN(value) ? 0 : value;
};

export const PageSettings = ({
  draft,
  dispatch,
  issues,
  pageId,
  onDeleted,
  onOpenSaveTemplate,
}: PageSettingsProps): ReactElement => {
  const { t } = useTranslator();
  const e = useEditorText();
  const grid = draft.layout === 'grid';
  const columns = pageColumns(draft.layoutData);
  const titleIssue = issues.some((issue) => issue.blockKey === null && issue.field === 'title');

  return (
    <div className="page-settings">
      <h2>{t('editor.pageSettings')}</h2>
      <Labelled
        id="page-title"
        label={t('sites.title')}
        error={
          titleIssue ? (
            <p className="field-error" role="alert">
              {t('field.errorRequired')}
            </p>
          ) : null
        }
      >
        <input
          id="page-title"
          type="text"
          value={draft.title}
          aria-invalid={titleIssue}
          onChange={(event) => {
            dispatch({ type: 'setPage', field: 'title', value: event.target.value });
          }}
        />
      </Labelled>
      <Labelled id="page-slug" label={t('sites.slug')}>
        <input
          id="page-slug"
          type="text"
          value={draft.slug}
          onChange={(event) => {
            dispatch({ type: 'setPage', field: 'slug', value: event.target.value });
          }}
        />
      </Labelled>
      <Checked
        id="page-public"
        label={t('common.public')}
        checked={draft.isPublic}
        onChange={(value) => {
          dispatch({ type: 'setPage', field: 'isPublic', value });
        }}
      />
      <Labelled id="page-layout" label={t('page.layout')}>
        <select
          id="page-layout"
          value={draft.layout}
          onChange={(event) => {
            dispatch({
              type: 'setPageLayout',
              layout: event.target.value === 'grid' ? 'grid' : 'normal',
            });
          }}
        >
          <option value="normal">{t('canvas.normal')}</option>
          <option value="grid">{t('canvas.grid')}</option>
        </select>
      </Labelled>
      {grid ? (
        <div className="page-grid-fields">
          <Labelled id="page-grid-columns" label={e('pageLayoutData.grid_columns')}>
            <input
              id="page-grid-columns"
              type="number"
              min={1}
              value={columns > 0 ? String(columns) : ''}
              onChange={(event) => {
                dispatch({
                  type: 'setPageLayoutData',
                  key: 'grid_columns',
                  value: numberFrom(event.target.value),
                });
              }}
            />
          </Labelled>
          <Labelled id="page-grid-column-gap" label={e('pageLayoutData.grid_column_gap')}>
            <input
              id="page-grid-column-gap"
              type="number"
              min={0}
              value={String(gridGap(draft.layoutData, 'grid_column_gap'))}
              onChange={(event) => {
                dispatch({
                  type: 'setPageLayoutData',
                  key: 'grid_column_gap',
                  value: numberFrom(event.target.value),
                });
              }}
            />
          </Labelled>
          <Labelled id="page-grid-row-gap" label={e('pageLayoutData.grid_row_gap')}>
            <input
              id="page-grid-row-gap"
              type="number"
              min={0}
              value={String(gridGap(draft.layoutData, 'grid_row_gap'))}
              onChange={(event) => {
                dispatch({
                  type: 'setPageLayoutData',
                  key: 'grid_row_gap',
                  value: numberFrom(event.target.value),
                });
              }}
            />
          </Labelled>
        </div>
      ) : null}
      {pageId === null ? null : (
        <DeleteControl
          label={t('page.deleteLabel')}
          rowName={draft.title}
          onConfirm={async () => {
            await api.deleteSitePage(pageId);
            onDeleted();
          }}
        />
      )}
      <button type="button" onClick={onOpenSaveTemplate}>
        {t('template.saveAs')}
      </button>
    </div>
  );
};
