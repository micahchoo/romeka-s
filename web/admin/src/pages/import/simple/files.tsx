/**
 * "Where are your files?" once a draft has them: one line with how many
 * files its folders hold, as the last look counted them, "Look at the files
 * again" for a folder that changed, and the next step. A draft is made over
 * its folder on the start page, so this screen never chooses one; files from
 * this computer are added here, and each one that arrives is counted with the
 * same look as the button's.
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
      <div className="guided-facts">
        <span>
          {t('guided.files.files')} <b>{number(files)}</b>
        </span>
        <span>
          {t('guided.files.from')} <b>{whereWords(draft.plan.sources, t, sources)}</b>
        </span>
        <button type="button" disabled={again.at === 'looking' || again.at === 'working'} onClick={look}>
          {again.at === 'looking' || again.at === 'working' ? t('guided.files.looking') : t('guided.files.again')}
        </button>
        <a className="button-link import-primary" href={placeHref(draft.id, { mode: 'simple', screen: 'structure' })}>
          {t('guided.files.next')}
        </a>
      </div>
      {sources
        .filter((source) => source.error !== null)
        .map((source) => (
          <p key={source.source} role="alert">
            {t('guided.files.error', { message: source.error ?? '' })}
          </p>
        ))}
      {again.at === 'working' ? <p role="status">{t('guided.files.againJob')}</p> : null}
      {again.at === 'done' ? <p role="status">{t('guided.files.againDone', { count: number(total(again.sources)) })}</p> : null}
      {again.at === 'failed' ? <ErrorNotice error={again.error} /> : null}
      <ComputerFiles lookAgain={look} />
    </>
  );
};

export default YourFiles;
