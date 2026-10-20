/**
 * Step 6, Check: a dry run of the plan as edited (`preview` with `check`),
 * the choice for rows that still have problems, and the button that starts
 * the import. Starting saves the plan first, so the run reads what the page
 * shows, then opens the Run step.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { CheckPreview, Preview } from '../../api/import-preview';
import { getImport, runImport, saveImportPlan } from '../../api/imports';
import type { ImportPlan } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { useImportText } from './messages';
import type { ImportMessageKey } from './messages';
import { PREVIEW_ERROR_TEXT, PreviewError } from './preview-error';
import { isConflict } from './run-api';
import { gridProblemsHref, tabHref } from './tabs';
import type { TabProps } from './tabs';
import { usePreview } from './use-preview';

/** How many unmatched files the step names before it counts the rest. */
const SAMPLE = 10;

const CHECK_ROWS = [
  { key: 'check.create', count: (check) => check.create, names: () => [] },
  { key: 'check.files', count: (check) => check.files, names: () => [] },
  { key: 'check.newSets', count: (check) => check.newSets.length, names: (check) => check.newSets },
  { key: 'check.existingSets', count: (check) => check.existingSets.length, names: (check) => check.existingSets },
  { key: 'check.newTypes', count: (check) => check.newTypes.length, names: (check) => check.newTypes },
  { key: 'check.newFields', count: (check) => check.newFields.length, names: (check) => check.newFields },
  { key: 'check.withProblems', count: (check) => check.withProblems, names: () => [] },
  { key: 'check.skipped', count: (check) => check.skipped, names: () => [] },
  { key: 'check.existing', count: (check) => check.existing, names: () => [] },
  { key: 'check.unmatched', count: (check) => check.unmatchedFiles, names: () => [] },
] as const satisfies readonly {
  readonly key: ImportMessageKey;
  readonly count: (check: CheckPreview) => number;
  readonly names: (check: CheckPreview) => readonly string[];
}[];

/** Why the button is off: the preview's refusal first, then what the check itself says. */
const blockedReason = (preview: Preview, check: CheckPreview, plan: ImportPlan): ImportMessageKey | null => {
  if (check.canRun) {
    return null;
  }
  if (preview.error !== undefined) {
    return PREVIEW_ERROR_TEXT[preview.error.key];
  }
  if (plan.problemRows === 'fix' && check.withProblems > 0) {
    return 'check.fixFirst';
  }
  if (check.create > 0) {
    return 'check.cannotRun';
  }
  return check.existing > 0 ? 'check.allExist' : 'check.nothing';
};

const DryRun = ({ check }: { readonly check: CheckPreview }): ReactElement => {
  const { t, number } = useImportText();
  return (
    <div className="table-scroll">
      <table className="import-check" aria-label={t('check.tableLabel')}>
        <tbody>
          {CHECK_ROWS.map((row) => (
            <tr key={row.key}>
              <th scope="row">{t(row.key)}</th>
              <td className="cell-num">{number(row.count(check))}</td>
              <td>{row.names(check).join(', ')}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

const ProblemChoice = ({ check, plan, editable, onChange, detail }: TabProps & { readonly check: CheckPreview }): ReactElement => {
  const { t, number } = useImportText();
  const choose = (problemRows: ImportPlan['problemRows']): void => {
    onChange((current) => ({ ...current, problemRows }));
  };
  return (
    <fieldset>
      <legend>{t('check.problemsLegend', { count: number(check.withProblems) })}</legend>
      <label className="import-radio">
        <input type="radio" name="problemRows" checked={plan.problemRows === 'leave'} disabled={!editable} onChange={() => { choose('leave'); }} />
        {t('check.leave')}
      </label>
      <label className="import-radio">
        <input type="radio" name="problemRows" checked={plan.problemRows === 'fix'} disabled={!editable} onChange={() => { choose('fix'); }} />
        {t('check.fix')}
      </label>
      <p>
        <a href={gridProblemsHref(detail.id)}>{t('check.showProblems')}</a>
      </p>
    </fieldset>
  );
};

const Unmatched = ({ check }: { readonly check: CheckPreview }): ReactElement | null => {
  const { t, number } = useImportText();
  if (check.unmatchedFiles === 0) {
    return null;
  }
  const shown = check.unmatchedSample.slice(0, SAMPLE);
  const rest = check.unmatchedFiles - shown.length;
  return (
    <>
      <h3>{t('check.unmatchedHeading')}</h3>
      <ul>
        {shown.map((path) => (
          <li key={path} className="import-mono">{path}</li>
        ))}
      </ul>
      {rest > 0 ? <p className="query-count">{t('check.more', { count: number(rest) })}</p> : null}
    </>
  );
};

type Start =
  | { readonly status: 'idle' }
  | { readonly status: 'starting' }
  | { readonly status: 'error'; readonly error: Error }
  | { readonly status: 'busy' };

const StartButton = ({
  props,
  check,
  reason,
  fresh,
}: {
  readonly props: TabProps;
  readonly check: CheckPreview;
  readonly reason: ImportMessageKey | null;
  /** Whether the check on screen answers the plan as it is now. */
  readonly fresh: boolean;
}): ReactElement => {
  const { t, number } = useImportText();
  const { detail, plan, onDetail } = props;
  const [start, setStart] = useState<Start>({ status: 'idle' });
  const run = (): void => {
    setStart({ status: 'starting' });
    saveImportPlan(api, detail.id, plan)
      .then(() => runImport(api, detail.id))
      .then(() => getImport(api, detail.id))
      .then(
        (next) => {
          onDetail(next);
          window.location.hash = tabHref(detail.id, 'run');
        },
        (error: unknown) => {
          setStart(isConflict(error) ? { status: 'busy' } : { status: 'error', error: toError(error) });
        },
      );
  };
  return (
    <>
      <div className="import-chips">
        <button type="button" className="button-primary" disabled={reason !== null || !fresh || start.status === 'starting'} onClick={run}>
          {start.status === 'starting' ? t('check.starting') : t('check.start', { count: number(check.create) })}
        </button>
        {reason === null ? null : <span className="import-hint">{t(reason)}</span>}
      </div>
      {start.status === 'busy' ? <p role="alert">{t('conflict.run')}</p> : null}
      {start.status === 'error' ? <ErrorNotice error={start.error} /> : null}
    </>
  );
};

export const CheckStep = (props: TabProps): ReactElement => {
  const { t, number } = useImportText();
  const { detail, plan, editable } = props;
  const state = usePreview(detail.id, { include: ['check'], plan });
  const preview = state.preview;
  const check = preview?.check;
  return (
    <section className="import-check-step">
      <p className="lede">{t('check.lede')}</p>
      {state.error === null ? null : <ErrorNotice error={state.error} />}
      <PreviewError preview={preview} />
      {preview === null || check === undefined ? (
        <p role="status">{t('archive.loadingPreview')}</p>
      ) : (
        <>
          <DryRun check={check} />
          {check.withProblems > 0 ? <ProblemChoice {...props} check={check} /> : null}
          <Unmatched check={check} />
          {check.openUndo === null ? null : (
            <p className="import-banner">
              {t('check.openUndo', { folder: check.openUndo.folder, items: number(check.openUndo.items) })}
            </p>
          )}
          {editable ? (
            <StartButton props={props} check={check} reason={blockedReason(preview, check, plan)} fresh={!state.loading} />
          ) : (
            <p>
              <a href={tabHref(detail.id, 'run')}>{t('check.started')}</a>
            </p>
          )}
        </>
      )}
    </section>
  );
};
