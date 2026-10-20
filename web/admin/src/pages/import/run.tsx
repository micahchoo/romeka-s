/**
 * Step 7, Run: the import's job as the server reports it, read again every
 * second while it runs or undoes. Each status has one view. An import never
 * runs twice; once it ends it can only be undone, and only while it holds
 * the undo window.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { getLeftOutCsv } from '../../api/imports';
import type { ImportDetail, ImportSummary, KeptKind, UndoReport } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { downloadText } from './grid-csv';
import { useImportText } from './messages';
import type { ImportMessageKey } from './messages';
import { tabHref } from './tabs';
import type { TabProps } from './tabs';
import { UndoPanel } from './undo';
import { useImportPoll } from './use-import-poll';

const JobLink = ({ detail }: { readonly detail: ImportDetail }): ReactElement | null => {
  const { t } = useImportText();
  return detail.jobId === null ? null : (
    <p>
      <a href={`#/jobs/${String(detail.jobId)}`}>{t('run.job', { id: detail.jobId })}</a>
    </p>
  );
};

const Progress = ({ detail, label }: { readonly detail: ImportDetail; readonly label: ImportMessageKey }): ReactElement => {
  const { t, number } = useImportText();
  const progress = detail.summary?.progress ?? { done: 0, total: 0 };
  return (
    <>
      <p role="status">{t(label, { done: number(progress.done), total: number(progress.total) })}</p>
      <progress max={Math.max(progress.total, 1)} value={progress.done} />
      <p className="query-count">{t('run.leave')}</p>
      <JobLink detail={detail} />
    </>
  );
};

/** The rows the import left out (skipped, left with problems, failed), as the server lists them. */
const downloadLeftOut = async ({ id, folder }: ImportDetail): Promise<void> => {
  downloadText(`${folder}-left-out.csv`, await getLeftOutCsv(api, id));
};

/** The one set the run filled, or the item set list when it filled several. */
const OpenSets = ({ summary }: { readonly summary: ImportSummary | null }): ReactElement | null => {
  const { t } = useImportText();
  const ids = summary?.itemSetIds ?? [];
  const [only] = ids;
  if (only === undefined) {
    return null;
  }
  return ids.length === 1 ? (
    <a href={`#/item-sets/${String(only)}`}>{t('run.openSet')}</a>
  ) : (
    <a href="#/item-sets">{t('run.openSets')}</a>
  );
};

/**
 * How the run ended. A job the server found dead wrote no reason of its own;
 * while the import holds the window the next step is its undo.
 */
const endingKey = ({ status, died, undoable }: ImportDetail): ImportMessageKey => {
  if (status !== 'failed') {
    return 'run.done';
  }
  return died !== null && undoable ? 'run.died' : 'run.failed';
};

const Ended = ({ detail, onUndo }: { readonly detail: ImportDetail; readonly onUndo: () => void }): ReactElement => {
  const { t, number } = useImportText();
  const [download, setDownload] = useState<{ readonly busy: boolean; readonly error: Error | null }>({
    busy: false,
    error: null,
  });
  const summary: ImportSummary | null = detail.summary;
  const left = summary?.left ?? 0;
  const existing = summary?.existing ?? 0;
  const save = (): void => {
    setDownload({ busy: true, error: null });
    downloadLeftOut(detail).then(
      () => { setDownload({ busy: false, error: null }); },
      (error: unknown) => { setDownload({ busy: false, error: toError(error) }); },
    );
  };
  return (
    <>
      <p role="status">
        {t(endingKey(detail))}{' '}
        {t('run.counts', {
          items: number(summary?.items ?? 0),
          files: number(summary?.files ?? 0),
          thumbnails: number(summary?.thumbnails ?? 0),
          left: number(left),
        })}
      </p>
      {existing > 0 ? <p>{t('run.existing', { count: number(existing) })}</p> : null}
      <p className="query-count">{t(detail.undoable ? 'run.undoWindow' : 'run.permanent')}</p>
      <JobLink detail={detail} />
      <div className="import-chips">
        {left > 0 ? (
          <button type="button" disabled={download.busy} onClick={save}>
            {download.busy ? t('run.downloading') : t('run.downloadLeft')}
          </button>
        ) : null}
        <OpenSets summary={summary} />
        {detail.undoable ? (
          <button type="button" onClick={onUndo}>
            {t('run.undo')}
          </button>
        ) : null}
        <a href="#/import">{t('run.another')}</a>
      </div>
      {download.error === null ? null : <ErrorNotice error={download.error} />}
    </>
  );
};

const KEPT_TEXT: Readonly<Record<KeptKind, { readonly key: ImportMessageKey; readonly href: string | null }>> = {
  itemSet: { key: 'undone.keptItemSet', href: '#/item-sets/' },
  template: { key: 'undone.keptTemplate', href: '#/resource-templates/' },
  field: { key: 'undone.keptField', href: null },
};

const Undone = ({ report }: { readonly report: UndoReport | null }): ReactElement => {
  const { t, number } = useImportText();
  if (report === null) {
    return <p role="status">{t('undone.noReport')}</p>;
  }
  return (
    <>
      <p role="status">
        {t('undone.done')}{' '}
        {t('undone.counts', {
          items: number(report.items),
          media: number(report.media),
          itemSets: number(report.itemSets),
          templates: number(report.templates),
          fields: number(report.fields),
        })}
      </p>
      {report.kept.length === 0 ? null : (
        <>
          <p>{t('undone.kept')}</p>
          <ul>
            {report.kept.map((kept) => {
              const { key, href } = KEPT_TEXT[kept.kind];
              const text = t(key, { id: kept.id });
              return (
                <li key={`${kept.kind}-${String(kept.id)}`}>
                  {href === null ? text : <a href={`${href}${String(kept.id)}`}>{text}</a>}
                </li>
              );
            })}
          </ul>
        </>
      )}
      <p>
        <a href="#/import">{t('run.another')}</a>
      </p>
    </>
  );
};

const NotStarted = ({ detail }: { readonly detail: ImportDetail }): ReactElement => {
  const { t } = useImportText();
  return (
    <p>
      {t('run.notStarted')} <a href={tabHref(detail.id, 'check')}>{t('run.toCheck')}</a>
    </p>
  );
};

export const RunStep = ({ detail, onDetail }: TabProps): ReactElement => {
  const poll = useImportPoll(detail, onDetail);
  const [asking, setAsking] = useState(false);
  const body = (): ReactElement => {
    switch (detail.status) {
      case 'draft':
        return <NotStarted detail={detail} />;
      case 'running':
        return <Progress detail={detail} label="run.importing" />;
      case 'undoing':
        return <Progress detail={detail} label="run.undoing" />;
      case 'undone':
        return <Undone report={detail.undoReport} />;
      case 'done':
      case 'failed':
        return asking && detail.undoable ? (
          <UndoPanel
            record={detail}
            onCancel={() => { setAsking(false); }}
            onStarted={() => {
              setAsking(false);
              poll.refresh();
            }}
          />
        ) : (
          <Ended detail={detail} onUndo={() => { setAsking(true); }} />
        );
    }
  };
  return (
    <section className="import-run">
      {poll.error === null ? null : <ErrorNotice error={poll.error} />}
      {body()}
    </section>
  );
};
