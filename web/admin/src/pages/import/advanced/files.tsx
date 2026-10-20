/**
 * Advanced, Files: where the files come from, the marked example with a
 * Remove for each mark, the pattern typed as a regex with what it fits, the
 * identical files, the files chart and the groups of items by their facts.
 */
import { lazy, Suspense } from 'react';
import type { ReactElement } from 'react';
import type { SourcePlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { listHref } from '../lists/address';
import { usePreview } from '../lists/use-preview';
import { whereWords } from '../words';
import { exampleOf, patternIsMarked, removeExample, removeMark } from './example';
import type { Mark } from './example';
import { useAdvancedText } from './messages';
import { PatternEditor } from './pattern-editor';
import { Setting } from './setting';
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
                  <b>{whereWords([source], t)}</b>{' '}
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

const markWords = (mark: Mark, t: ReturnType<typeof useAdvancedText>['t']): string =>
  mark.kind === 'number'
    ? t('adv.files.example.number', { text: mark.text })
    : t('adv.files.example.words', { words: mark.words.map((word) => `“${word}”`).join(', ') });

const Example = (): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const example = exampleOf(draft.plan);
  const stored = draft.plan.example !== null;
  const followed = example !== null && patternIsMarked(draft.plan, example);
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
              <button type="button" disabled={!draft.editable} onClick={() => { draft.change(removeExample); }}>
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
                      draft.change((plan) => removeMark(plan, at));
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
  const { pattern, caseFold } = draft.plan.grouping;
  return (
    <div className="adv-section">
      <h4>{t('adv.files.sources.title')}</h4>
      <Sources />
      <h4>{t('adv.files.pattern.title')}</h4>
      <Example />
      <PatternEditor key={JSON.stringify([pattern, caseFold])} />
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
