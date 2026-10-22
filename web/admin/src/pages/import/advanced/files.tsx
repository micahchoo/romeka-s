/**
 * Advanced, Files: where the files come from, files added from this
 * computer (looked at again as each arrives), then for the folder picked
 * (`SeriesPicker`) the marked example with a Remove for each mark and the
 * pattern typed as a regex with what it fits, then for every folder the
 * identical files, the files chart and the groups of items by their facts.
 */
import { lazy, Suspense } from 'react';
import type { ReactElement } from 'react';
import type { LevelId, SourcePlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { exampleKept, exampleOf, patternIsMarked, removeExample, removeMark } from '../example';
import type { Mark } from '../example';
import { listHref } from '../lists/address';
import { useLookAgain } from '../look-again';
import { usePreview } from '../lists/use-preview';
import { ComputerFiles } from '../upload/computer-files';
import { whereWords } from '../words';
import { useAdvancedText } from './messages';
import { PatternEditor } from './pattern-editor';
import { Setting } from './setting';
import { usePickedSeries } from '../picked-series';
import { NoSeries, SeriesPicker } from './series-picker';
import './advanced.css';

const IdenticalFiles = lazy(() => import('../lists/identical'));
const FilesChart = lazy(() => import('../lists/histogram'));
const Groups = lazy(() => import('../lists/groups'));

const Sources = (): ReactElement => {
  const { t, number } = useAdvancedText();
  const { plan } = useDraft();
  const read = usePreview([], {});
  const stateOf = (source: SourcePlan) => (read.status === 'ready' ? read.data.sources.find((one) => one.source === source.id) : undefined);
  return (
    <Setting label={t('adv.files.sources.label')} plain={t('adv.files.sources.plain')}>
      {() => (
        <>
          {read.status === 'error' ? <ErrorNotice error={read.error} /> : null}
          <ul className="adv-list">
            {plan.sources.map((source) => {
              const state = stateOf(source);
              return (
                <li key={source.id}>
                  <b>{whereWords([source], t, read.status === 'ready' ? read.data.sources : [])}</b>{' '}
                  <code>{source.id}</code>{' '}
                  {state === undefined ? null : state.error !== null ? (
                    <span role="alert">{t('adv.files.sources.error', { message: state.error })}</span>
                  ) : state.scanned === null ? (
                    <span className="muted">{t('adv.files.sources.notRead')}</span>
                  ) : (
                    <span className="muted">{state.files === 1 ? t('adv.files.sources.filesOne') : t('adv.files.sources.files', { count: number(state.files) })}</span>
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}
    </Setting>
  );
};

const markWords = (mark: Mark, t: ReturnType<typeof useAdvancedText>['t']): string => {
  switch (mark.kind) {
    case 'number':
      return t('adv.files.example.number', { text: mark.text });
    case 'letters':
      return t('adv.files.example.letters', { text: mark.text });
    case 'words':
      return t('adv.files.example.words', { words: mark.words.map((word) => `“${word}”`).join(', ') });
  }
};

const Example = ({ level }: { readonly level: LevelId }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const example = exampleOf(draft.plan, level);
  const stored = exampleKept(draft.plan, level);
  const followed = example !== null && patternIsMarked(draft.plan, level, example);
  return (
    <Setting label={t('adv.files.example.label')} plain={t('adv.files.example.plain')}>
      {() => {
        if (!stored) {
          return <p className="adv-plain">{t('adv.files.example.none')}</p>;
        }
        if (example === null) {
          return (
            <>
              <p>{t('adv.files.example.unread')}</p>
              <button type="button" disabled={!draft.editable} onClick={() => { draft.change((plan) => removeExample(plan, level)); }}>
                {t('adv.files.example.removeAll')}
              </button>
            </>
          );
        }
        return (
          <>
            {example.path === null ? null : (
              <p>
                {t('adv.files.example.file')} <code>{example.path}</code>
              </p>
            )}
            <ul className="adv-list">
              {example.marks.map((mark, at) => (
                <li key={at}>
                  {markWords(mark, t)}{' '}
                  <button
                    type="button"
                    disabled={!draft.editable}
                    aria-label={t('adv.files.example.removeOne', { mark: markWords(mark, t) })}
                    onClick={() => {
                      draft.change((plan) => removeMark(plan, level, at));
                    }}
                  >
                    {t('adv.files.example.remove')}
                  </button>
                </li>
              ))}
            </ul>
            <p className="adv-plain">{followed ? t('adv.files.example.writes') : t('adv.files.example.typed')}</p>
          </>
        );
      }}
    </Setting>
  );
};

export const AdvancedFiles = (): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const series = usePickedSeries();
  const { look, lookAgain } = useLookAgain();
  return (
    <div className="adv-section">
      <h4>{t('adv.files.sources.title')}</h4>
      <Sources />
      <ComputerFiles lookAgain={lookAgain} />
      {look.at === 'failed' ? <ErrorNotice error={look.error} /> : null}
      <h4>{t('adv.files.pattern.title')}</h4>
      {series === null ? (
        <NoSeries />
      ) : (
        <>
          <SeriesPicker picked={series} />
          <Example level={series.id} />
          <PatternEditor key={JSON.stringify([series.id, series.grouping.pattern, series.grouping.caseFold])} level={series.id} />
        </>
      )}
      <Suspense fallback={null}>
        <h4>{t('adv.files.facts.title')}</h4>
        <IdenticalFiles />
        <FilesChart />
        <Groups />
      </Suspense>
      <p>
        <a href={listHref(draft.id, { mode: 'advanced', screen: 'all-files' }, {})}>{t('adv.files.every')}</a>
      </p>
    </div>
  );
};

export default AdvancedFiles;
