/**
 * `#/import/<id>/<step>`: one import's seven steps as tabs, with Back and Next.
 * The page holds the plan as edited; every step reads it and changes it
 * through `onChange`, and `usePlanSave` writes it back while it is a draft.
 */
import { Suspense, useCallback, useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { deleteImport, getImport } from '../../api/imports';
import type { ImportDetail, ImportId, ImportPlan } from '../../api/imports';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useImportText } from './messages';
import { STEPS } from './steps';
import { TABS, tabHref } from './tabs';
import type { TabSlug } from './tabs';
import { usePlanSave } from './use-plan-save';
import type { SaveState } from './use-plan-save';

const SaveStatus = ({ state }: { readonly state: SaveState }): ReactElement | null => {
  const { t } = useImportText();
  switch (state.status) {
    case 'idle':
      return null;
    case 'saving':
      return <span className="import-save" role="status">{t('archive.saving')}</span>;
    case 'saved':
      return <span className="import-save" role="status">{t('archive.saved')}</span>;
    case 'error':
      return (
        <span className="import-save" role="alert">
          {t('archive.saveFailed', { message: state.error.message })}
        </span>
      );
  }
};

const StepTabs = ({ id, tab }: { readonly id: ImportId; readonly tab: TabSlug }): ReactElement => {
  const { t } = useImportText();
  return (
    <nav className="tabs import-steps" aria-label={t('archive.stepsLabel')}>
      <ul>
        {TABS.map((entry, index) => (
          <li key={entry.slug}>
            <a href={tabHref(id, entry.slug)} aria-current={entry.slug === tab ? 'step' : undefined}>
              {t('archive.stepName', { number: index + 1, name: t(entry.titleKey) })}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
};

const StepNav = ({ id, tab }: { readonly id: ImportId; readonly tab: TabSlug }): ReactElement => {
  const { t } = useImportText();
  const index = TABS.findIndex((entry) => entry.slug === tab);
  const go = (to: number): void => {
    const target = TABS[to];
    if (target !== undefined) {
      window.location.hash = tabHref(id, target.slug);
    }
  };
  return (
    <div className="import-step-nav">
      <button type="button" disabled={index <= 0} onClick={() => { go(index - 1); }}>
        {t('archive.back')}
      </button>
      <button
        type="button"
        className="button-primary"
        disabled={index >= TABS.length - 1}
        onClick={() => { go(index + 1); }}
      >
        {t('archive.next')}
      </button>
    </div>
  );
};

interface EditorProps {
  readonly tab: TabSlug;
  readonly problemsOnly: boolean;
}

const DraftEditor = ({ initial, tab, problemsOnly }: EditorProps & { readonly initial: ImportDetail }): ReactElement => {
  const { t } = useImportText();
  const [detail, setDetail] = useState<ImportDetail>(initial);
  const [plan, setPlan] = useState<ImportPlan>(initial.plan);
  const editable = detail.status === 'draft';
  const saveState = usePlanSave(detail.id, plan, editable);
  const onChange = useCallback(
    (change: (current: ImportPlan) => ImportPlan) => {
      if (editable) {
        setPlan(change);
      }
    },
    [editable],
  );
  const Step = STEPS[tab];
  return (
    <section className="import-draft">
      <div className="import-title">
        <h2>{t('archive.heading', { folder: detail.folder })}</h2>
        <SaveStatus state={saveState} />
      </div>
      {editable ? null : <p className="import-banner">{t('archive.readOnly')}</p>}
      <StepTabs id={detail.id} tab={tab} />
      <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
        <Step
          detail={detail}
          plan={plan}
          editable={editable}
          onChange={onChange}
          onDetail={setDetail}
          problemsOnly={problemsOnly}
        />
      </Suspense>
      <StepNav id={detail.id} tab={tab} />
      {editable ? (
        <DeleteControl
          label={t('archive.deleteDraft')}
          rowName={detail.folder}
          onConfirm={async () => {
            await deleteImport(api, detail.id);
            window.location.hash = '#/import';
          }}
        />
      ) : null}
    </section>
  );
};

export const ImportDraft = ({ id, tab, problemsOnly }: EditorProps & { readonly id: ImportId }): ReactElement => {
  const { t } = useImportText();
  const state = useAsync(() => getImport(api, id), [id]);
  if (state.status === 'loading') {
    return <p role="status">{t('archive.loading')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <DraftEditor initial={state.data} tab={tab} problemsOnly={problemsOnly} />;
};
