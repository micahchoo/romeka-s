/**
 * "Change all N at once": one change to every item or file a filter selects,
 * saved as one `many` decision that names each entry it hits.
 *
 * The person picks the change; the bar then reads every page the filter
 * selects, with the count read so far and a way to stop, and names the first
 * items before anything changes. Only "Change them" adds the decision, so a
 * filter that selects the wrong items shows it before it costs anything.
 */
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import type { Change } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { useListsText } from './messages';
import { manyDecision } from './gather';
import type { Gathered } from './gather';

export type ChangeChoice =
  | { readonly kind: 'button'; readonly label: string; readonly change: Change }
  | {
      readonly kind: 'select';
      /** The first option, which changes nothing ("Put them in an item set..."). */
      readonly label: string;
      readonly options: readonly { readonly label: string; readonly change: Change }[];
    };

export interface ChangeManyProps {
  /** What the filter selects now. */
  readonly count: number;
  readonly noun: 'items' | 'files';
  readonly choices: readonly ChangeChoice[];
  readonly gather: (onRead: (read: number, total: number) => void, signal: AbortSignal) => Promise<Gathered>;
}

type Step =
  | { readonly at: 'idle' }
  | { readonly at: 'reading'; readonly label: string; readonly read: number; readonly total: number }
  | { readonly at: 'confirm'; readonly label: string; readonly change: Change; readonly gathered: Gathered }
  | { readonly at: 'failed'; readonly error: Error }
  | { readonly at: 'done'; readonly label: string; readonly count: number };

export const ChangeMany = ({ count, noun, choices, gather }: ChangeManyProps): ReactElement => {
  const { t, number } = useListsText();
  const draft = useDraft();
  const [step, setStep] = useState<Step>({ at: 'idle' });
  const stop = useRef<AbortController | null>(null);
  useEffect(() => () => {
    stop.current?.abort();
  }, []);

  const start = (label: string, change: Change): void => {
    const controller = new AbortController();
    stop.current = controller;
    setStep({ at: 'reading', label, read: 0, total: count });
    gather((read, total) => {
      setStep({ at: 'reading', label, read, total });
    }, controller.signal).then(
      (gathered) => {
        if (!controller.signal.aborted) {
          setStep({ at: 'confirm', label, change, gathered });
        }
      },
      (error: unknown) => {
        if (!controller.signal.aborted) {
          setStep({ at: 'failed', error: toError(error) });
        }
      },
    );
  };

  const cancel = (): void => {
    stop.current?.abort();
    setStep({ at: 'idle' });
  };

  const many = noun === 'items' ? t('lists.many.items', { count: number(count) }) : t('lists.many.files', { count: number(count) });

  if (step.at === 'reading') {
    return (
      <div className="import-bulk" role="group" aria-label={many} aria-busy="true">
        <p role="status" className="import-slow">{t('lists.many.reading', { read: number(step.read), total: number(step.total) })}</p>
        <button type="button" onClick={cancel}>
          {t('lists.many.stop')}
        </button>
      </div>
    );
  }
  if (step.at === 'confirm') {
    const { gathered } = step;
    const more = gathered.count - gathered.names.length;
    return (
      <div className="import-bulk" role="group" aria-label={many}>
        <p>
          <b>{step.label}</b>{' '}
          {more > 0
            ? t('lists.many.namesMore', { count: number(gathered.count), names: gathered.names.join(', '), more: number(more) })
            : t('lists.many.names', { count: number(gathered.count), names: gathered.names.join(', ') })}
        </p>
        <div className="import-row-actions">
          <button
            type="button"
            className="import-primary"
            disabled={!draft.editable || gathered.entries.length === 0}
            onClick={() => {
              draft.change((plan) => ({ ...plan, decisions: [...plan.decisions, manyDecision(gathered.entries, step.change)] }));
              setStep({ at: 'done', label: step.label, count: gathered.count });
            }}
          >
            {t('lists.many.apply', { count: number(gathered.count) })}
          </button>
          <button type="button" onClick={cancel}>
            {t('lists.many.cancel')}
          </button>
        </div>
      </div>
    );
  }
  return (
    <div className="import-bulk" role="group" aria-label={many}>
      <b>{many}</b>
      {choices.map((choice) =>
        choice.kind === 'button' ? (
          <button
            key={choice.label}
            type="button"
            disabled={!draft.editable}
            onClick={() => {
              start(choice.label, choice.change);
            }}
          >
            {choice.label}
          </button>
        ) : (
          <select
            key={choice.label}
            aria-label={choice.label}
            value=""
            disabled={!draft.editable || choice.options.length === 0}
            onChange={(event) => {
              const picked = choice.options[Number(event.target.value)];
              if (picked !== undefined) {
                start(picked.label, picked.change);
              }
            }}
          >
            <option value="">{choice.label}</option>
            {choice.options.map((option, index) => (
              <option key={option.label} value={index}>
                {option.label}
              </option>
            ))}
          </select>
        ),
      )}
      {draft.editable ? null : <p className="muted">{t('shell.closed')}</p>}
      {step.at === 'failed' ? <ErrorNotice error={step.error} /> : null}
      {step.at === 'done' ? (
        <p role="status" className="ok-text">
          {t('lists.many.done', { change: step.label, count: number(step.count) })}
        </p>
      ) : null}
    </div>
  );
};
