/**
 * "Where are your files?" once a draft has them: how many files each folder
 * holds, as the last look counted them, and "Look at the files again" for a
 * folder that changed. A draft is made over its folder on the start page, so
 * this screen never chooses one.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { SourceState } from '../../../api/import-sections';
import type { ScanResult } from '../../../api/import-view';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { whereWords } from '../words';
import { useSimpleText } from './messages';
import './simple.css';

type Again = { readonly at: 'idle' } | { readonly at: 'looking' } | { readonly at: 'done'; readonly result: ScanResult } | { readonly at: 'failed'; readonly error: Error };

const total = (sources: readonly SourceState[]): number => sources.reduce((sum, source) => sum + source.files, 0);

export const YourFiles = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [again, setAgain] = useState<Again>({ at: 'idle' });
  const read = usePreview(['histogram'], {});
  if (draft.plan.sources.length === 0) {
    return (
      <div className="import-note">
        <p>
          <b>{t('guided.files.none')}</b>
        </p>
        <p>
          <a href="#/import">{t('guided.files.choose')}</a>
        </p>
      </div>
    );
  }
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const sources = read.data.sources;
  const look = (): void => {
    setAgain({ at: 'looking' });
    importApi.scan(draft.id).then(
      (result) => {
        setAgain({ at: 'done', result });
      },
      (error: unknown) => {
        setAgain({ at: 'failed', error: toError(error) });
      },
    );
  };
  const files = total(sources);
  return (
    <>
      <div className="import-note">
        <p>
          <b>
            {files === 1
              ? t('guided.files.countOne', { where: whereWords(draft.plan.sources, t) })
              : t('guided.files.count', { count: number(files), where: whereWords(draft.plan.sources, t) })}
          </b>
        </p>
        {sources
          .filter((source) => source.error !== null)
          .map((source) => (
            <p key={source.source} role="alert">
              {t('guided.files.error', { message: source.error ?? '' })}
            </p>
          ))}
      </div>
      <p className="import-row-actions">
        <a className="button-link import-primary" href={placeHref(draft.id, { mode: 'simple', screen: 'found' })}>
          {t('guided.files.next')}
        </a>
        <button type="button" disabled={again.at === 'looking'} onClick={look}>
          {again.at === 'looking' ? t('guided.files.looking') : t('guided.files.again')}
        </button>
      </p>
      {again.at === 'done' ? (
        <p role="status">
          {again.result.kind === 'job'
            ? t('guided.files.againJob')
            : t('guided.files.againDone', { count: number(total(again.result.sources)) })}
        </p>
      ) : null}
      {again.at === 'failed' ? <ErrorNotice error={again.error} /> : null}
    </>
  );
};

export default YourFiles;
