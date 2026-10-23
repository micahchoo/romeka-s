/**
 * Check: one line of counts, then every question beside the inspector of the
 * one selected. There is one question for each kind of problem and each
 * anomaly, not one for each item; facts that need no answer (files Omeka
 * refuses, small things the run takes care of) are rows too. The inspector
 * shows one example and another, opens all of them, and takes one answer for
 * all, with the suggested answer marked. The run uses the suggested answer
 * for a question the person skips, so "Use the N suggested answers" answers
 * every open question at once.
 */
import { Fragment, useState } from 'react';
import type { ReactElement } from 'react';
import type { FlagCount, FlagSample } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { listHref } from '../lists/address';
import { fileName, recordName, recordTitle } from '../lists/record-words';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { answerOf, checkFactsOf, suggestedOf, withAnswer, withSuggestions, wordsOf } from './causes';
import type { CheckFacts } from './causes';
import { readState, withState } from './example-state';
import { MasterDetail, Segmented } from './master-detail';
import { useSimpleText } from './messages';
import type { SimpleMessageKey, SimpleTranslator } from './messages';
import { QUESTION_SECTIONS, factsOf, isBlocking, openOf, questionQuery, questionsOf } from './questions';
import type { CauseId, CheckQuestion } from './questions';
import './simple.css';

interface Cause {
  readonly id: CauseId;
  readonly count: number;
  /** Examples the server drew: names or values, already in words. */
  readonly examples: readonly string[];
  /** Records the flag is on, for an example drawn from the items list. */
  readonly kind: FlagCount['kind'] | null;
}

/** A row of the list: a question to answer, or a fact the run acts on by itself. */
type Row =
  | { readonly kind: 'question'; readonly key: string; readonly cause: Cause }
  | { readonly kind: 'refused'; readonly key: string; readonly flag: FlagCount; readonly examples: readonly string[] }
  | { readonly kind: 'handled'; readonly key: string; readonly flag: FlagCount };

type QuestionState = 'mustFix' | 'open' | 'answered' | 'suggested';

const STATE_CHIPS: Readonly<Record<QuestionState | 'fact', { readonly key: SimpleMessageKey; readonly className: string }>> = {
  mustFix: { key: 'guided.check.chipMustFix', className: 'guided-chip is-warn' },
  open: { key: 'guided.check.chipOpen', className: 'guided-chip' },
  answered: { key: 'guided.check.chipAnswered', className: 'guided-chip is-ok' },
  suggested: { key: 'guided.check.chipSuggested', className: 'guided-chip is-ok' },
  fact: { key: 'guided.check.chipFact', className: 'guided-chip' },
};

const sampleWords = (sample: FlagSample): string | null => {
  const { subject } = sample;
  if (typeof subject === 'object' && 'entry' in subject) {
    return 'file' in subject.entry ? fileName(subject.entry.file) : subject.entry.row.key.replace(/#\d+$/, '');
  }
  return sample.detail;
};

/** Kinds whose detail is the ids of the Omeka items a record matched: the person knows the record by its code, not by those ids. */
const MATCHED: readonly FlagCount['kind'][] = ['exists', 'many-matches'];

const examplesOf = (flag: FlagCount): readonly string[] => flag.samples.map(sampleWords).filter((words): words is string => words !== null);

const questionState = (cause: Cause, answered: readonly string[], answer: string, suggested: string): QuestionState => {
  if (isBlocking(cause.id)) {
    return 'mustFix';
  }
  if (!answered.includes(cause.id)) {
    return 'open';
  }
  return answer === suggested ? 'suggested' : 'answered';
};

const causeSay = (cause: Cause, words: ReturnType<typeof wordsOf>, t: SimpleTranslator['t'], number: SimpleTranslator['number']): string =>
  t(cause.count === 1 ? words.sayOne : words.say, { count: number(cause.count), problem: cause.kind === null ? '' : t(`problem.${cause.kind}`) });

const refusedSay = (count: number, t: SimpleTranslator['t'], number: SimpleTranslator['number']): string =>
  t(count === 1 ? 'guided.check.refusedOne' : 'guided.check.refused', { count: number(count) });

const handledSay = (flag: FlagCount, t: SimpleTranslator['t'], number: SimpleTranslator['number']): string =>
  t(flag.kind === 'not-media' ? 'guided.check.handledNotMedia' : 'guided.check.handledSpaces', { count: number(flag.count) });

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
  if (MATCHED.includes(kind)) {
    const title = recordTitle(record);
    return (
      <>
        {title === null ? t('guided.check.example', { name: recordName(record) }) : t('guided.check.exampleOf', { name: recordName(record), value: title })}
        {record.matches.map((id) => (
          <Fragment key={id}>
            {' '}
            <a href={`#/items/${String(id)}`}>{t('guided.check.omekaItem', { id: String(id) })}</a>
          </Fragment>
        ))}
      </>
    );
  }
  const detail = record.flags.find((flag) => flag.kind === kind)?.detail ?? null;
  return <>{detail === null ? t('guided.check.example', { name: recordName(record) }) : t('guided.check.exampleOf', { name: recordName(record), value: detail })}</>;
};

/** One question: what was counted, an example, and one answer for all of them. */
const Question = ({ cause, facts, shown, onAnother }: { readonly cause: Cause; readonly facts: CheckFacts; readonly shown: number; readonly onAnother: () => void }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const words = wordsOf(cause.id, draft.plan, facts);
  const answered = readState(draft.plan).answered.includes(cause.id);
  const answer = answerOf(draft.plan, cause.id, facts);
  const suggested = suggestedOf(draft.plan, cause.id, facts);
  const blocking = isBlocking(cause.id);
  const example = cause.examples.length === 0 ? null : (cause.examples[shown % cause.examples.length] ?? null);
  const answer1 = (value: string): void => {
    draft.change((plan) => withState(withAnswer(plan, cause.id, value, facts), { answered: [...readState(plan).answered.filter((id) => id !== cause.id), cause.id] }));
  };
  const all = cause.kind === null ? null : listHref(draft.id, { mode: 'simple', screen: 'all-items' }, { problem: cause.kind });
  const say = causeSay(cause, words, t, number);
  return (
    <section aria-label={say} className="guided-inspect" data-decision={`cause:${cause.id}`}>
      <h4>{say}</h4>
      {words.why === null ? null : <p className="muted">{t(words.why)}</p>}
      <p className="import-row-actions">
        {example !== null ? <span>{t('guided.check.example', { name: example })}</span> : cause.kind !== null ? <RecordExample kind={cause.kind} at={shown % Math.max(1, Math.min(3, cause.count))} /> : null}
        {cause.count > 1 ? (
          <button type="button" onClick={onAnother}>
            {t('guided.check.another')}
          </button>
        ) : null}
        {all === null ? null : <a href={all}>{t('guided.check.showAll', { count: number(cause.count) })}</a>}
        {cause.id === 'no-file' || cause.id === 'itemsWithoutRow' ? <a href={placeHref(draft.id, { mode: 'simple', screen: 'pair' })}>{t('guided.check.pair')}</a> : null}
        {cause.id === 'unknown-type' ? <a href={placeHref(draft.id, { mode: 'simple', screen: 'structure' })}>{t('guided.check.chooseTypes')}</a> : null}
      </p>
      {blocking ? (
        <p role="alert">{t('guided.check.mustFix')}</p>
      ) : (
        <p className="import-row-actions">
          <Segmented
            label={t('guided.check.answerFor', { count: number(cause.count) })}
            segments={words.options.map((option) => ({ value: option.value, label: t(option.label), suggested: option.value === suggested }))}
            value={answer}
            disabled={!draft.editable}
            suggestedWord={t('guided.check.suggested')}
            onChange={answer1}
          />
          {answered ? null : (
            <button
              type="button"
              disabled={!draft.editable}
              onClick={() => {
                answer1(answer);
              }}
            >
              {t('guided.check.useOne')}
            </button>
          )}
        </p>
      )}
    </section>
  );
};

/** A file the list names that this Omeka refuses: the person has no choice here, only the way to the settings. */
const Refused = ({ count, examples }: { readonly count: number; readonly examples: readonly string[] }): ReactElement => {
  const { t, number } = useSimpleText();
  const say = refusedSay(count, t, number);
  return (
    <section aria-label={say} className="guided-inspect" data-decision="fact:listed-not-media">
      <h4>{say}</h4>
      <p className="muted">{t('guided.check.refusedWhy')}</p>
      <p className="import-row-actions">
        {examples.length === 0 ? null : <span>{t('guided.check.refusedExample', { names: examples.join(', ') })}</span>}
        <a href="#/settings">{t('guided.check.refusedSettings')}</a>
      </p>
    </section>
  );
};

const Summary = ({ items, open, answered, onUseAll }: { readonly items: number; readonly open: number; readonly answered: number; readonly onUseAll: () => void }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  return (
    <div className="guided-facts">
      <span>
        {t('guided.check.toAdd')} <b>{number(items)}</b>
      </span>
      <span>
        {t('guided.check.openCount')} <b>{number(open)}</b>
      </span>
      <span>
        {t('guided.check.answeredCount')} <b>{number(answered)}</b>
      </span>
      {open === 0 ? (
        <span className="guided-chip is-ok">{t('guided.check.allAnswered')}</span>
      ) : (
        <button type="button" className="import-primary" disabled={!draft.editable} onClick={onUseAll}>
          {open === 1 ? t('guided.check.useAllOne') : t('guided.check.useAll', { count: number(open) })}
        </button>
      )}
    </div>
  );
};

const Help = (): ReactElement => {
  const { t } = useSimpleText();
  return (
    <details className="guided-help">
      <summary>{t('guided.list.help')}</summary>
      <p>{t('guided.check.once')}</p>
      <p>{t('guided.check.helpStates')}</p>
    </details>
  );
};

export const Check = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [seed, setSeed] = useState(0);
  const [shown, setShown] = useState<Readonly<Record<string, number>>>({});
  const read = usePreview([...QUESTION_SECTIONS, 'histogram', 'listCandidates'], { ...questionQuery(3), seed });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { flags, identical, unpaired, entries, check } = read.data;
  const facts = checkFactsOf(draft.plan, read.data);
  const examplesFor = (question: CheckQuestion): readonly string[] => {
    switch (question.id) {
      case 'identical':
        return identical.sets.items.map((set) => set.paths.map((path) => path.slice(path.lastIndexOf('/') + 1)).join(' = '));
      case 'unmatchedFiles':
        return entries.items.flatMap((entry) => ('file' in entry.entry ? [fileName(entry.entry.file)] : []));
      case 'itemsWithoutRow':
        return unpaired.files.items.map((file) => fileName(file.file));
      default:
        return question.kind === null || MATCHED.includes(question.kind) ? [] : question.samples.map(sampleWords).filter((words): words is string => words !== null);
    }
  };
  const causes: Cause[] = questionsOf(draft.plan, read.data).map((question) => ({ id: question.id, count: question.count, examples: examplesFor(question), kind: question.kind }));
  const { answered } = readState(draft.plan);
  const open = openOf(draft.plan, causes);
  const done = causes.filter((cause) => !isBlocking(cause.id) && answered.includes(cause.id));
  const items = check.counts.filter((count) => count.kind === 'item' && count.does !== 'leave').reduce((sum, count) => sum + count.count, 0);
  const rows: Row[] = [
    ...causes.map((cause) => ({ kind: 'question' as const, key: `cause:${cause.id}`, cause })),
    ...factsOf(flags).refused.map((flag) => ({ kind: 'refused' as const, key: `fact:${flag.kind}`, flag, examples: examplesOf(flag) })),
    ...factsOf(flags).handled.map((flag) => ({ kind: 'handled' as const, key: `handled:${flag.kind}/${flag.slot ?? ''}`, flag })),
  ];
  const useAll = (): void => {
    draft.change((plan) => {
      const { plan: suggested } = withSuggestions(plan, open.map((cause) => cause.id), facts);
      return withState(suggested, { answered: [...new Set([...readState(plan).answered, ...open.map((cause) => cause.id)])] });
    });
  };
  const nameOf = (row: Row): string => {
    if (row.kind === 'question') {
      return t(wordsOf(row.cause.id, draft.plan, facts).name);
    }
    return t(`problem.${row.flag.kind}`);
  };
  const chipOf = (row: Row): QuestionState | 'fact' =>
    row.kind === 'question'
      ? questionState(row.cause, answered, answerOf(draft.plan, row.cause.id, facts), suggestedOf(draft.plan, row.cause.id, facts))
      : 'fact';
  return (
    <div className="guided-dials" aria-busy={read.busy}>
      <Summary items={items} open={open.length} answered={done.length} onUseAll={useAll} />
      {rows.length === 0 ? (
        <p className="ok-text">{t('guided.check.nothing')}</p>
      ) : (
        <MasterDetail
          label={t('guided.check.questions')}
          initial={open[0] === undefined ? null : `cause:${open[0].id}`}
          rows={rows.map((row) => {
            const chip = STATE_CHIPS[chipOf(row)];
            return {
              key: row.key,
              name: <span data-decision={row.kind === 'question' ? `cause:${row.cause.id}` : undefined}>{nameOf(row)}</span>,
              facts: (
                <>
                  <b>{number(row.kind === 'question' ? row.cause.count : row.flag.count)}</b>
                  <span className={chip.className}>{t(chip.key)}</span>
                </>
              ),
            };
          })}
        >
          {(key) => {
            const row = rows.find((one) => one.key === key);
            if (row === undefined) {
              return null;
            }
            if (row.kind === 'refused') {
              return <Refused key={key} count={row.flag.count} examples={row.examples} />;
            }
            if (row.kind === 'handled') {
              const say = handledSay(row.flag, t, number);
              return (
                <section key={key} aria-label={say} className="guided-inspect">
                  <h4>{say}</h4>
                </section>
              );
            }
            const { cause } = row;
            return (
              <Question
                key={key}
                cause={cause}
                facts={facts}
                shown={shown[cause.id] ?? 0}
                onAnother={() => {
                  const at = (shown[cause.id] ?? 0) + 1;
                  if (cause.examples.length > 0 && at % cause.examples.length === 0) {
                    setSeed(seed + 1);
                  }
                  setShown({ ...shown, [cause.id]: at });
                }}
              />
            );
          }}
        </MasterDetail>
      )}
      <Help />
    </div>
  );
};

export default Check;
