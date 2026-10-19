/**
 * The inspector when no block is selected: the page's own members. The title,
 * slug, visibility and layout edit the page row. The page's `o:layout_data`
 * (the grid and the advanced template and class) is the schema's
 * `pageLayoutData`, rendered by the same form a block's styling uses, so each
 * field carries its help and the list cannot drift from the renderer's. That
 * form is the fields chunk, loaded on demand as the block inspector loads it.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { api } from '../../../../api/client';
import type { PageId, SiteId } from '../../../../api/types';
import { DeleteControl } from '../../../../app/delete';
import { useTranslator } from '../../../../i18n';
import type { DraftAction, PageDraft, ValidationIssue } from '../draft';
import { Checked, Labelled } from '../Labelled';
import type { EditorOptions } from '../options';
import { layoutObject } from '../values';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import { editor } from '../../../../i18n/namespaces/editor';

const LayoutDataForm = lazy(() =>
  import('../fields').then((module) => ({ default: module.LayoutDataForm })),
);

export interface PageSettingsProps {
  readonly draft: PageDraft;
  readonly dispatch: (action: DraftAction) => void;
  readonly issues: readonly ValidationIssue[];
  readonly pageId: PageId | null;
  readonly siteId: SiteId;
  readonly options: EditorOptions;
  /** Called after the page row is deleted, so the shell can return to the site. */
  readonly onDeleted: () => void;
  /** Open the shell's save-as-template dialog. */
  readonly onOpenSaveTemplate: () => void;
}

export const PageSettings = ({
  draft,
  dispatch,
  issues,
  pageId,
  siteId,
  options,
  onDeleted,
  onOpenSaveTemplate,
}: PageSettingsProps): ReactElement => {
  const { t } = useTranslator(common, sites, editor);
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
      <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
        <LayoutDataForm
          idPrefix="page-layout"
          owner="page"
          schema={draft.registry.pageLayoutData}
          values={layoutObject(draft.layoutData)}
          grid={draft.layout === 'grid'}
          siteId={siteId}
          options={options}
          onChange={(values) => {
            for (const [key, value] of Object.entries(values)) {
              dispatch({ type: 'setPageLayoutData', key, value });
            }
          }}
        >
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
        </LayoutDataForm>
      </Suspense>
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
