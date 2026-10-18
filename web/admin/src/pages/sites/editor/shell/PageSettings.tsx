/**
 * The inspector when no block is selected: the page's own members. The title,
 * slug and visibility edit the page row; the layout picker and the grid fields
 * edit the page's `o:layout_data`, which the renderer reads for every page.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { PageId } from '../../../../api/types';
import { DeleteControl } from '../../../../app/delete';
import { CheckboxField, TextField } from '../../../../app/fields';
import { useTranslator } from '../../../../i18n';
import { gridGap, pageColumns } from '../canvas/grid';
import type { DraftAction, PageDraft, ValidationIssue } from '../draft';

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
  const grid = draft.layout === 'grid';
  const columns = pageColumns(draft.layoutData);
  const titleIssue = issues.some((issue) => issue.blockKey === null && issue.field === 'title');

  return (
    <div className="page-settings">
      <h2>{t('editor.pageSettings')}</h2>
      <TextField
        id="page-title"
        label={t('sites.title')}
        value={draft.title}
        onChange={(value) => {
          dispatch({ type: 'setPage', field: 'title', value });
        }}
      />
      {titleIssue ? (
        <p className="field-error" role="alert">
          {t('field.errorRequired')}
        </p>
      ) : null}
      <TextField
        id="page-slug"
        label={t('sites.slug')}
        value={draft.slug}
        onChange={(value) => {
          dispatch({ type: 'setPage', field: 'slug', value });
        }}
      />
      <CheckboxField
        id="page-public"
        label={t('common.public')}
        checked={draft.isPublic}
        onChange={(value) => {
          dispatch({ type: 'setPage', field: 'isPublic', value });
        }}
      />
      <p>
        <label htmlFor="page-layout">{t('page.layout')}</label>
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
      </p>
      {grid ? (
        <div className="page-grid-fields">
          <p>
            <label htmlFor="page-grid-columns">{t('pageLayoutData.grid_columns')}</label>
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
          </p>
          <p>
            <label htmlFor="page-grid-column-gap">{t('pageLayoutData.grid_column_gap')}</label>
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
          </p>
          <p>
            <label htmlFor="page-grid-row-gap">{t('pageLayoutData.grid_row_gap')}</label>
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
          </p>
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
