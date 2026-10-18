/**
 * The job update surface.
 *
 * `Admin/JobController` has no edit action, and `JobAdapter::hydrate` is empty,
 * so no member of a request body reaches a job's columns. `PATCH /api/jobs/<id>`
 * still finds the row, checks the admin grant and answers its representation
 * (`crates/omeka-api/src/write/jobs.rs`). The form carries the update and names
 * why it has no control: every column is written by the dispatcher.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { api } from '../../api/client';
import type { Job } from '../../api/types';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { useSave } from '../../app/save';
import { useTranslator } from '../../i18n';

export const JobUpdateForm = ({
  job,
  onSaved,
}: {
  readonly job: Job;
  readonly onSaved: (job: Job) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    save(async () => {
      onSaved(await api.updateJob(job.id));
    });
  };

  return (
    <form onSubmit={submit}>
      <p>{t('jobs.noWritableMember')}</p>
      <SaveStatus state={saveState} />
      <SubmitRow label={t('jobs.update')} busy={saveState.status === 'saving'} />
    </form>
  );
};
