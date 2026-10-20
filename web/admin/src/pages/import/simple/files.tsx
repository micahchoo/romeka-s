/**
 * "Where are your files?" once a draft has them: how many files each folder
 * holds, as the last look counted them, and "Look at the files again" for a
 * folder that changed. A draft is made over its folder on the start page, so
 * this screen never chooses one; files from this computer are added here, and
 * each one that arrives is counted with the same look as the button's.
 */
import type { ReactElement } from 'react';
import type { SourceState } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { useLookAgain } from '../look-again';
import { placeHref } from '../screens';
import { ComputerFiles } from '../upload/computer-files';
import { whereWords } from '../words';
import { useSimpleText } from './messages';
import './simple.css';

const total = (sources: readonly SourceState[]): number => sources.reduce((sum, source) => sum + source.files, 0);

export const YourFiles = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const { look: again, lookAgain: look } = useLookAgain();
  const read = usePreview(['histogram'], {});
  if (draft.plan.sources.length === 0) {
    return (
      <>
        <div className="import-note">
          <p>
            <b>{t('guided.files.none')}</b>
          </p>
          <p>
            <a href="#/import">{t('guided.files.choose')}</a>
          </p>
        </div>
        <ComputerFiles lookAgain={look} />
      </>
    );
  }
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const sources = read.data.sources;
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
      <ComputerFiles lookAgain={look} />
    </>
  );
};

export default YourFiles;
