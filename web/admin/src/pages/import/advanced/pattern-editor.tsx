/**
 * The pattern, typed as a regex. What a typed pattern would do is counted by
 * the server before it is used: the preview runs on the plan with the typed
 * pattern in place of the saved one (`options.plan`, not saved), so the
 * person reads "N of M files fit" and sees the groups on a real name, then
 * presses "Use this pattern". The counts read unfitting files as left out,
 * whatever the answer to them is, so a fit is never hidden by that answer.
 *
 * A folder of a proposal is counted as Simple counts a marked example: the
 * typed pattern is an owned write (`writes.ts`), so the plan tried holds
 * `own` on the folder, and the proposal's `own` node is the folder as typed.
 * Its counts are the folder's alone; the sample name is the folder's first
 * file, relative to the folder, as the pattern reads it. The archive as found
 * is one folder over everything, so there the whole plan's counts are its own.
 */
import { useEffect, useId, useState } from 'react';
import type { ReactElement } from 'react';
import type { ImportPlan, LevelId } from '../../../api/import-plan';
import type { PatternState } from '../../../api/import-model';
import { importApi } from '../../../api/import-routes';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { listHref } from '../lists/address';
import { exampleOf, patternOf } from '../example';
import { useAdvancedText } from './messages';
import type { AdvancedTranslator } from './messages';
import { partsOf } from './pattern-parts';
import { setAnomalies, setPattern } from './plan-edits';
import { ownScopeOf, standingOf } from './writes';
import { Setting } from './setting';
import { seriesOf } from '../series';

/** Long enough to count once per pause in typing, short enough to read as live. */
export const TRY_DELAY_MS = 400;

interface Fit {
  readonly pattern: PatternState;
  readonly files: number;
  readonly unfit: number;
  readonly items: number;
  readonly sample: string | null;
  /** The folder the counts are of; `null`: the whole plan. */
  readonly folder: string | null;
}

type Counting = { readonly status: 'counting'; readonly last: Fit | null } | { readonly status: 'ready'; readonly fit: Fit } | { readonly status: 'error'; readonly error: Error };

const countFit = async (id: Parameters<typeof importApi.preview>[0], plan: ImportPlan, sample: string | null): Promise<Fit> => {
  const counted = setAnomalies(plan, { unmatchedFiles: 'leave' });
  const [all, unfit] = await Promise.all([
    importApi.preview(id, ['entries'], { plan: counted, entries: { folder: '', perPage: 1 } }),
    importApi.preview(id, ['entries', 'histogram'], { plan: counted, entries: { reason: 'unmatched', perPage: 1 } }),
  ]);
  const first = all.entries.items[0]?.entry;
  return {
    pattern: unfit.pattern,
    files: all.entries.total,
    unfit: unfit.entries.total,
    items: unfit.histogram.filter((bucket) => bucket.files !== '0').reduce((sum, bucket) => sum + bucket.count, 0),
    sample: sample ?? (first !== undefined && 'file' in first ? first.file.path : null),
    folder: null,
  };
};

/** The path as the pattern reads it: relative to the scope folder that holds it. */
const inFolder = (path: string, folder: string): string => (folder === '' || !path.startsWith(`${folder}/`) ? path : path.slice(folder.length + 1));

/** The counts of the series `level` as `plan` writes it: the `own` node of the plan's proposal. */
const countFolder = async (id: Parameters<typeof importApi.preview>[0], plan: ImportPlan, level: LevelId, sample: string | null, gone: (folder: string) => string): Promise<Fit> => {
  const folder = ownScopeOf(plan, level)?.folder ?? '';
  const [data, { parseProposal }] = await Promise.all([
    importApi.preview(id, ['proposal', 'entries'], { plan: setAnomalies(plan, { unmatchedFiles: 'leave' }), entries: { folder, perPage: 1 } }),
    import('../../../api/import-proposal'),
  ]);
  const node = parseProposal(data.proposal).nodes.find((one) => one.kind === 'own' && one.level === level);
  if (node?.kind !== 'own') {
    throw new Error(gone(folder));
  }
  const first = data.entries.items[0]?.entry;
  return {
    pattern: data.patterns?.find((one) => one.series === level)?.pattern ?? data.pattern,
    files: node.items.files.of,
    unfit: node.items.files.of - node.items.files.n,
    items: node.items.items,
    sample: sample ?? (first !== undefined && 'file' in first ? inFolder(first.file.path, folder) : null),
    folder,
  };
};

const problemOf = (state: PatternState, t: AdvancedTranslator['t']): string | null => {
  if (state === 'missing') {
    return t('adv.files.pattern.missing');
  }
  if (state === 'noIdentifier') {
    return t('adv.files.pattern.noIdentifier');
  }
  return 'syntax' in state ? t('adv.files.pattern.syntax', { message: state.syntax }) : null;
};

const Parts = ({ pattern, path }: { readonly pattern: string; readonly path: string }): ReactElement => {
  const { t } = useAdvancedText();
  const parts = partsOf(pattern, path);
  if (parts === null) {
    return <p className="adv-plain">{t('adv.files.parts.none', { path })}</p>;
  }
  const named = parts.filter((part) => part.group !== null);
  return (
    <div className="adv-parts">
      <p className="adv-parts-name">
        {parts.map((part, at) =>
          part.group === null ? (
            <span key={at}>{part.text}</span>
          ) : (
            <mark key={at} className={`adv-group adv-group-${part.group === 'identifier' || part.group === 'part' ? part.group : 'other'}`}>
              {part.text}
            </mark>
          ),
        )}
      </p>
      <ul className="adv-parts-key">
        {named.map((part, at) => (
          <li key={at}>
            <code>{part.group}</code> {part.text}
          </li>
        ))}
      </ul>
    </div>
  );
};

/** The pattern of the series `level`. Keyed by the series and its saved pattern, so a pattern saved from elsewhere (a mark removed, Simple) starts it again. */
export const PatternEditor = ({ level }: { readonly level: LevelId }): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const saved = seriesOf(draft.plan, level).grouping;
  const example = exampleOf(draft.plan, level);
  const [text, setText] = useState(saved.pattern ?? '');
  const [caseFold, setCaseFold] = useState(saved.caseFold);
  const [counting, setCounting] = useState<Counting>({ status: 'counting', last: null });
  const fieldId = useId();
  const typed = text.trim() === '' ? null : text;
  const tried = setPattern(draft.plan, level, typed, caseFold);
  const changed = typed !== saved.pattern || caseFold !== saved.caseFold;
  const triedKey = JSON.stringify(tried);

  useEffect(() => {
    let current = true;
    setCounting((last) => ({ status: 'counting', last: last.status === 'ready' ? last.fit : last.status === 'counting' ? last.last : null }));
    const timer = setTimeout(() => {
      const sample = example?.path ?? null;
      const counted =
        standingOf(draft.plan, level) === 'asFound'
          ? countFit(draft.id, tried, sample)
          : countFolder(draft.id, tried, level, sample, (folder) => t('adv.files.fit.gone', { folder }));
      counted.then(
        (fit) => {
          if (current) {
            setCounting({ status: 'ready', fit });
          }
        },
        (error: unknown) => {
          if (current) {
            setCounting({ status: 'error', error: toError(error) });
          }
        },
      );
    }, TRY_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
    // `triedKey` holds the plan tried; the plan object is new on every render.
  }, [draft.id, triedKey, draft.detail.revision]);

  const fit = counting.status === 'ready' ? counting.fit : counting.status === 'counting' ? counting.last : null;
  const problem = fit === null ? null : problemOf(fit.pattern, t);
  const usable = changed && counting.status === 'ready' && problem === null && draft.editable;
  const marked = example === null ? null : patternOf(example.marks);
  const filesPlace = { mode: 'advanced', screen: 'all-files' } as const;
  return (
    <Setting label={t('adv.files.pattern.label')} plain={t('adv.files.pattern.plain')}>
      {(described) => (
        <form
          className="adv-pattern"
          onSubmit={(event) => {
            event.preventDefault();
            if (usable) {
              draft.change((plan) => setPattern(plan, level, typed, caseFold));
            }
          }}
        >
          <label htmlFor={fieldId}>{t('adv.files.pattern.field')}</label>
          <textarea
            id={fieldId}
            rows={2}
            spellCheck={false}
            value={text}
            aria-describedby={described}
            aria-invalid={problem !== null}
            disabled={!draft.editable}
            onChange={(event) => {
              setText(event.target.value);
            }}
          />
          <label className="adv-check">
            <input
              type="checkbox"
              checked={caseFold}
              disabled={!draft.editable}
              onChange={(event) => {
                setCaseFold(event.target.checked);
              }}
            />{' '}
            {t('adv.files.pattern.caseFold')}
          </label>
          <div className="adv-fit" aria-live="polite" aria-busy={counting.status === 'counting'}>
            {counting.status === 'error' ? <p role="alert">{t('adv.files.fit.unread', { message: counting.error.message })}</p> : null}
            {fit === null && counting.status === 'counting' ? <p className="import-slow">{t('adv.files.fit.counting')}</p> : null}
            {fit !== null && problem !== null ? <p role="alert" className="adv-problem">{problem}</p> : null}
            {fit !== null && problem === null ? (
              <p>
                {t('adv.files.fit.count', { fit: number(fit.files - fit.unfit), files: number(fit.files), items: number(fit.items) })}{' '}
                {fit.unfit === 0 ? null : (
                  <>
                    {fit.unfit === 1 ? t('adv.files.fit.unfitOne') : t('adv.files.fit.unfit', { count: number(fit.unfit) })}{' '}
                    {changed ? null : <a href={listHref(draft.id, filesPlace, fit.folder === null ? { reason: 'unmatched' } : { reason: 'unmatched', folder: fit.folder })}>{t('adv.files.fit.showUnfit')}</a>}
                  </>
                )}
              </p>
            ) : null}
            {fit !== null && problem === null && typed !== null && fit.sample !== null ? <Parts pattern={typed} path={fit.sample} /> : null}
          </div>
          <p className="import-row-actions">
            <button type="submit" disabled={!usable}>
              {t('adv.files.pattern.use')}
            </button>
            {marked !== null && marked !== text ? (
              <button
                type="button"
                disabled={!draft.editable}
                onClick={() => {
                  setText(marked);
                }}
              >
                {t('adv.files.pattern.fromMarks')}
              </button>
            ) : null}
            {changed ? (
              <button
                type="button"
                onClick={() => {
                  setText(saved.pattern ?? '');
                  setCaseFold(saved.caseFold);
                }}
              >
                {t('adv.files.pattern.back')}
              </button>
            ) : null}
          </p>
          <p className="adv-plain">{changed ? t('adv.files.pattern.notUsed') : t('adv.files.pattern.inUse')}</p>
        </form>
      )}
    </Setting>
  );
};
