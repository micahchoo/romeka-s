/**
 * Check: one card for each kind of problem and each anomaly, not one for each
 * item. A card says how many, shows one example and another, opens all of
 * them, and takes one answer for all, with the suggested answer marked. The
 * run uses the suggested answer for a card the person skips, so "Use the N
 * suggested answers" answers every card at once.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { FlagCount, FlagSample } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { listHref } from '../lists/address';
import { fileName, recordName } from '../lists/record-words';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { CAUSE_ORDER, DEFAULT_POLICY, HANDLED, answerOf, isBlocking, suggestedOf, withAnswer, wordsOf } from './causes';
import type { CauseId } from './causes';
import { readState, withState } from './example-state';
import { useSimpleText } from './messages';
import './simple.css';

interface Cause {
  readonly id: CauseId;
  readonly count: number;
  /** Examples the server drew: names or values, already in words. */
  readonly examples: readonly string[];
  /** Records the flag is on, for an example drawn from the items list. */
  readonly kind: FlagCount['kind'] | null;
}

const sampleWords = (sample: FlagSample): string | null => {
  const { subject } = sample;
  if (typeof subject === 'object' && 'entry' in subject) {
    return 'file' in subject.entry ? fileName(subject.entry.file) : subject.entry.row.key.replace(/#\d+$/, '');
  }
  return sample.detail;
};

const RecordExample = ({ kind, at }: { readonly kind: FlagCount['kind']; readonly at: number }): ReactElement | null => {
  const { t } = useSimpleText();
  const read = usePreview(['records'], { records: { kind: 'item', flag: kind, page: at + 1, perPage: 1 } });
  if (read.status !== 'ready') {
    return null;
  }
  const record = read.data.records.items[0];
  if (record === undefined) {
    return null;
  }
  const detail = record.flags.find((flag) => flag.kind === kind)?.detail ?? null;
  return <>{detail === null ? t('guided.check.example', { name: recordName(record) }) : t('guided.check.exampleOf', { name: recordName(record), value: detail })}</>;
};

const Card = ({ cause, shown, onAnother }: { readonly cause: Cause; readonly shown: number; readonly onAnother: () => void }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const words = wordsOf(cause.id, draft.plan);
  const answered = readState(draft.plan).answered.includes(cause.id);
  const answer = answerOf(draft.plan, cause.id);
  const suggested = suggestedOf(draft.plan, cause.id);
  const blocking = isBlocking(cause.id);
  const example = cause.examples.length === 0 ? null : (cause.examples[shown % cause.examples.length] ?? null);
  const answer1 = (value: string): void => {
    draft.change((plan) => withState(withAnswer(plan, cause.id, value), { answered: [...readState(plan).answered.filter((id) => id !== cause.id), cause.id] }));
  };
  const all = cause.kind === null ? null : listHref(draft.id, { mode: 'simple', screen: 'all-items' }, { problem: cause.kind });
  return (
    <article className={`guided-ask${answered ? ' guided-done' : ''}`} data-decision={`cause:${cause.id}`} aria-label={t(cause.count === 1 ? words.sayOne : words.say, { count: number(cause.count), problem: cause.kind === null ? '' : t(`problem.${cause.kind}`) })}>
      <p>
        <b>{t(cause.count === 1 ? words.sayOne : words.say, { count: number(cause.count), problem: cause.kind === null ? '' : t(`problem.${cause.kind}`) })}</b>{' '}
        {words.why === null ? null : t(words.why)}
      </p>
      <p className="import-row-actions">
        {example !== null ? <span>{t('guided.check.example', { name: example })}</span> : cause.kind !== null ? <RecordExample kind={cause.kind} at={shown % Math.max(1, Math.min(3, cause.count))} /> : null}
        {cause.count > 1 ? (
          <button type="button" onClick={onAnother}>
            {t('guided.check.another')}
          </button>
        ) : null}
        {all === null ? null : <a href={all}>{t('guided.check.showAll', { count: number(cause.count) })}</a>}
        {cause.id === 'no-file' || cause.id === 'itemsWithoutRow' ? <a href={placeHref(draft.id, { mode: 'simple', screen: 'pair' })}>{t('guided.check.pair')}</a> : null}
        {cause.id === 'unknown-type' ? <a href={placeHref(draft.id, { mode: 'simple', screen: 'where' })}>{t('guided.check.chooseTypes')}</a> : null}
      </p>
      {blocking ? (
        <p role="alert">{t('guided.check.mustFix')}</p>
      ) : (
        <fieldset className="guided-opts" aria-label={t('guided.check.answerFor', { count: number(cause.count) })}>
          {words.options.map((option) => (
            <label key={option.value}>
              <input
                type="radio"
                name={`cause-${cause.id}`}
                value={option.value}
                checked={answer === option.value}
                disabled={!draft.editable}
                onChange={() => {
                  answer1(option.value);
                }}
              />
              <span>
                {t(option.label)}
                {option.value === suggested ? <span className="guided-pill">{t('guided.check.suggested')}</span> : null}
              </span>
            </label>
          ))}
        </fieldset>
      )}
      {blocking ? null : answered ? (
        <p className="ok-text">{t('guided.check.answered')}</p>
      ) : (
        <button type="button" disabled={!draft.editable} onClick={() => { answer1(answer); }}>
          {t('guided.check.useOne')}
        </button>
      )}
    </article>
  );
};

const order = (id: CauseId): number => CAUSE_ORDER.indexOf(id);

/** A kind the run takes care of under its default policy; with another policy it is a question again. */
const handledBy = (flag: FlagCount): boolean => HANDLED.some((kind) => kind === flag.kind && flag.policy === DEFAULT_POLICY[kind]);

export const Check = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [seed, setSeed] = useState(0);
  const [shown, setShown] = useState<Readonly<Record<string, number>>>({});
  const read = usePreview(['flags', 'identical', 'unpaired', 'entries', 'check'], {
    identical: { perPage: 3 },
    unpaired: { rows: { perPage: 1 }, files: { perPage: 3 } },
    entries: { reason: 'unmatched', perPage: 3 },
    seed,
  });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { flags, identical, unpaired, entries, check } = read.data;
  const byKind = new Map<FlagCount['kind'], { count: number; samples: FlagSample[] }>();
  for (const flag of flags) {
    if (flag.count === 0 || handledBy(flag)) {
      continue;
    }
    const kept = byKind.get(flag.kind) ?? { count: 0, samples: [] };
    kept.count += flag.count;
    kept.samples.push(...flag.samples);
    byKind.set(flag.kind, kept);
  }
  const causes: Cause[] = [...byKind.entries()].map(([kind, found]) => {
    const examples = found.samples.map(sampleWords).filter((words): words is string => words !== null);
    return { id: kind, count: found.count, examples, kind };
  });
  if (identical.files > 0) {
    causes.push({ id: 'identical', count: identical.files, examples: identical.sets.items.map((set) => set.paths.map((path) => path.slice(path.lastIndexOf('/') + 1)).join(' = ')), kind: null });
  }
  if (entries.total > 0) {
    causes.push({ id: 'unmatchedFiles', count: entries.total, examples: entries.items.flatMap((entry) => ('file' in entry.entry ? [fileName(entry.entry.file)] : [])), kind: null });
  }
  if (draft.plan.sheets.length > 0 && unpaired.files.total > 0) {
    causes.push({ id: 'itemsWithoutRow', count: unpaired.files.total, examples: unpaired.files.items.map((file) => fileName(file.file)), kind: null });
  }
  causes.sort((a, b) => order(a.id) - order(b.id));
  const { answered } = readState(draft.plan);
  const open = causes.filter((cause) => !isBlocking(cause.id) && !answered.includes(cause.id));
  const items = check.counts.filter((count) => count.kind === 'item' && count.does !== 'leave').reduce((sum, count) => sum + count.count, 0);
  const handled = flags.filter((flag) => handledBy(flag) && flag.count > 0);
  return (
    <>
      <div className="guided-ask" aria-busy={read.busy}>
        <p className="guided-big">
          {items === 1 ? t('guided.check.readyOne') : t('guided.check.ready', { count: number(items) })}{' '}
          {open.length === 0 ? null : <b>{open.length === 1 ? t('guided.check.openOne') : t('guided.check.open', { count: number(open.length) })}</b>}
        </p>
        <p className="muted">{open.length === 0 ? t('guided.check.allAnswered') : t('guided.check.once')}</p>
        {open.length > 1 ? (
          <button
            type="button"
            className="import-primary"
            disabled={!draft.editable}
            onClick={() => {
              draft.change((plan) => withState(plan, { answered: [...new Set([...readState(plan).answered, ...open.map((cause) => cause.id)])] }));
            }}
          >
            {t('guided.check.useAll', { count: number(open.length) })}
          </button>
        ) : null}
      </div>
      {causes.length === 0 ? <p className="ok-text">{t('guided.check.nothing')}</p> : null}
      <div className="guided-cards">
        {causes.map((cause) => (
          <Card
            key={cause.id}
            cause={cause}
            shown={shown[cause.id] ?? 0}
            onAnother={() => {
              const at = (shown[cause.id] ?? 0) + 1;
              if (cause.examples.length > 0 && at % cause.examples.length === 0) {
                setSeed(seed + 1);
              }
              setShown({ ...shown, [cause.id]: at });
            }}
          />
        ))}
      </div>
      {handled.length === 0 ? null : (
        <details>
          <summary>{handled.length === 1 ? t('guided.check.handledOne') : t('guided.check.handled', { count: number(handled.length) })}</summary>
          <ul>
            {handled.map((flag) => (
              <li key={`${flag.kind}/${flag.slot ?? ''}`}>{t(flag.kind === 'not-media' ? 'guided.check.handledNotMedia' : 'guided.check.handledSpaces', { count: number(flag.count) })}</li>
            ))}
          </ul>
        </details>
      )}
    </>
  );
};

export default Check;
