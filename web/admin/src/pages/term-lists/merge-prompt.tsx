/**
 * One group of alike spellings and the choice it asks for: keep one spelling
 * and merge the others into it, or say they are different. A merge first asks
 * the server what it would change, then writes on confirmation.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { markDistinct, mergeValues } from '../../api/tags';
import type { Rewrite, ValueCount, ValueScope } from '../../api/tags';
import { ErrorNotice } from '../../app/errors';
import { useTranslator } from '../../i18n';
import { useTermListText } from './messages';
import { RewriteCounts } from './rewrite-counts';

type Step =
  | { readonly kind: 'choose' }
  | { readonly kind: 'busy' }
  | { readonly kind: 'confirm'; readonly preview: Rewrite }
  | { readonly kind: 'error'; readonly error: Error };

const asError = (error: unknown): Error =>
  error instanceof Error ? error : new Error(String(error));

export const MergePrompt = ({
  scope,
  group,
  onDone,
}: {
  readonly scope: ValueScope;
  readonly group: readonly ValueCount[];
  /** Called with what to tell the user once the merge or the answer is stored. */
  readonly onDone: (message: string) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const tl = useTermListText();
  const [kept, setKept] = useState(group[0]?.value ?? '');
  const [step, setStep] = useState<Step>({ kind: 'choose' });
  const from = group.map((count) => count.value).filter((value) => value !== kept);

  const run = (task: () => Promise<void>): void => {
    setStep({ kind: 'busy' });
    task().catch((error: unknown) => {
      setStep({ kind: 'error', error: asError(error) });
    });
  };
  const preview = (): void => {
    run(async () => {
      setStep({
        kind: 'confirm',
        preview: await mergeValues(api, scope, { from, to: kept, dryRun: true }),
      });
    });
  };
  const merge = (): void => {
    run(async () => {
      const done = await mergeValues(api, scope, { from, to: kept, dryRun: false });
      onDone(tl('merge.merged', { value: kept, changed: done.changed }));
    });
  };
  const different = (): void => {
    run(async () => {
      await markDistinct(
        api,
        scope,
        group.map((count) => count.value),
      );
      onDone(tl('merge.remembered'));
    });
  };
  const name = `merge-${group.map((count) => count.value).join('|')}`;

  return (
    <section className="merge-prompt" aria-label={tl('merge.title')}>
      <h3>{tl('merge.title')}</h3>
      <p>{tl('merge.intro')}</p>
      <fieldset
        className="merge-choices"
        aria-label={tl('merge.title')}
        disabled={step.kind !== 'choose' && step.kind !== 'error'}
      >
        {group.map((count) => (
          <label key={count.value} className="merge-choice">
            <input
              type="radio"
              name={name}
              value={count.value}
              checked={kept === count.value}
              onChange={() => {
                setKept(count.value);
                setStep({ kind: 'choose' });
              }}
            />
            <span>{tl('merge.keep', { value: count.value })}</span>
            <span className="merge-count">{tl('merge.usedBy', { count: count.resources })}</span>
          </label>
        ))}
      </fieldset>
      {step.kind === 'confirm' ? (
        <>
          <RewriteCounts rewrite={step.preview} />
          {step.preview.deprecated.length === 0 ? null : (
            <p className="merge-deprecates">
              {tl('merge.deprecates', {
                values: step.preview.deprecated.map((value) => `“${value}”`).join(', '),
                value: kept,
              })}
            </p>
          )}
          <p className="merge-actions">
            <button type="button" className="button-primary" onClick={merge}>
              {tl('merge.confirm')}
            </button>
            <button
              type="button"
              onClick={() => {
                setStep({ kind: 'choose' });
              }}
            >
              {t('action.cancel')}
            </button>
          </p>
        </>
      ) : (
        <p className="merge-actions">
          <button
            type="button"
            className="button-primary"
            disabled={step.kind === 'busy'}
            onClick={preview}
          >
            {tl('merge.into', { value: kept })}
          </button>
          <button type="button" disabled={step.kind === 'busy'} onClick={different}>
            {tl('merge.different')}
          </button>
        </p>
      )}
      {step.kind === 'error' ? <ErrorNotice error={step.error} /> : null}
    </section>
  );
};
