/**
 * The line over each screen: what the saved answers make, counted by the
 * server (`check` of the preview), and in Simple the questions Check counts
 * as open (`./simple/questions`), read again after each save.
 * Simple says it in words; Advanced adds the changes and the action. The line
 * keeps the last counts while new ones load, so it never blinks.
 */
import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../api/import-routes';
import type { ImportPlan } from '../../api/import-plan';
import type { Check } from '../../api/import-sections';
import { toError } from '../../app/useAsync';
import { useDraft } from './draft-context';
import { useImportText } from './messages';
import { placeHref } from './screens';
import { QUESTION_SECTIONS, openOf, questionQuery, questionsOf } from './simple/questions';
import type { QuestionReads } from './simple/questions';

interface Counts {
  readonly items: number;
  readonly left: number;
  /** The questions Check counts as open; Advanced does not count them. */
  readonly open: number | null;
  readonly stops: number;
  readonly action: Check['action'];
}

interface Reads {
  readonly check: Check;
  readonly questions: QuestionReads | null;
}

const countsOf = (plan: ImportPlan, { check, questions }: Reads): Counts => {
  const items = check.counts.filter((count) => count.kind === 'item');
  const sum = (rows: typeof items): number => rows.reduce((total, row) => total + row.count, 0);
  return {
    items: sum(items.filter((count) => count.does !== 'leave')),
    left: sum(items.filter((count) => count.does === 'leave')),
    open: questions === null ? null : openOf(plan, questionsOf(plan, questions)).length,
    stops: check.blocking.length,
    action: check.action,
  };
};

type Read = { readonly reads: Reads } | { readonly error: Error } | null;

export const StatusLine = (): ReactElement | null => {
  const { t, number } = useImportText();
  const draft = useDraft();
  const { id, detail, plan, place } = draft;
  const [read, setRead] = useState<Read>(null);
  const revision = detail.revision;
  const simple = place.mode === 'simple';
  useEffect(() => {
    let current = true;
    const asked: Promise<Reads> = simple
      ? importApi.preview(id, QUESTION_SECTIONS, questionQuery(1)).then((preview) => ({ check: preview.check, questions: preview }))
      : importApi.preview(id, ['check']).then((preview) => ({ check: preview.check, questions: null }));
    asked.then(
      (reads) => {
        if (current) {
          setRead({ reads });
        }
      },
      (error: unknown) => {
        if (current) {
          setRead({ error: toError(error) });
        }
      },
    );
    return () => {
      current = false;
    };
  }, [id, revision, simple]);

  if (read === null) {
    return null;
  }
  if ('error' in read) {
    return <p className="import-status-line muted">{t('line.unread', { message: read.error.message })}</p>;
  }
  const counts = countsOf(plan, read.reads);
  if (!simple) {
    return (
      <p className="import-status-line" aria-live="polite">
        {t('advanced.line', {
          items: number(counts.items),
          left: number(counts.left),
          changes: number(plan.decisions.length),
          action: counts.action,
        })}
      </p>
    );
  }
  const { open } = counts;
  if (open === null) {
    return null;
  }
  return (
    <p className="import-status-line" aria-live="polite">
      <b>{counts.items === 1 ? t('line.itemsOne') : t('line.items', { count: number(counts.items) })}</b>{' '}
      {counts.left === 0 ? null : <>{t('line.left', { count: number(counts.left) })} </>}
      {open === 0 ? (
        counts.stops === 0 ? <>{t('line.nothing')} </> : null
      ) : (
        <>
          <a href={placeHref(id, { mode: 'simple', screen: 'check' })}>
            {open === 1 ? t('line.openOne') : t('line.open', { count: number(open) })}
          </a>
          {'. '}
        </>
      )}
      {counts.stops === 0 ? null : <>{t('line.stops', { count: number(counts.stops) })} </>}
      <span className="muted">{t('line.notYet')}</span>
    </p>
  );
};
