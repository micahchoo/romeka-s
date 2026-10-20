/**
 * The line over each screen: what the saved answers make, counted by the
 * server (`check` and `flags` of the preview), read again after each save.
 * Simple says it in words; Advanced adds the changes and the action. The line
 * keeps the last counts while new ones load, so it never blinks.
 */
import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../api/import-routes';
import type { Check, FlagCount } from '../../api/import-sections';
import { toError } from '../../app/useAsync';
import { useDraft } from './draft-context';
import { useImportText } from './messages';
import { placeHref } from './screens';

interface Counts {
  readonly items: number;
  readonly left: number;
  readonly kinds: number;
  readonly stops: number;
  readonly action: Check['action'];
}

const countsOf = (check: Check, flags: readonly FlagCount[]): Counts => {
  const items = check.counts.filter((count) => count.kind === 'item');
  const sum = (rows: typeof items): number => rows.reduce((total, row) => total + row.count, 0);
  return {
    items: sum(items.filter((count) => count.does !== 'leave')),
    left: sum(items.filter((count) => count.does === 'leave')),
    kinds: flags.filter((flag) => flag.count > 0).length,
    stops: check.blocking.length,
    action: check.action,
  };
};

type Read = { readonly counts: Counts } | { readonly error: Error } | null;

export const StatusLine = (): ReactElement | null => {
  const { t, number } = useImportText();
  const draft = useDraft();
  const { id, detail, plan, place } = draft;
  const [read, setRead] = useState<Read>(null);
  const revision = detail.revision;
  useEffect(() => {
    let current = true;
    importApi.preview(id, ['check', 'flags']).then(
      (preview) => {
        if (current) {
          setRead({ counts: countsOf(preview.check, preview.flags) });
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
  }, [id, revision]);

  if (read === null) {
    return null;
  }
  if ('error' in read) {
    return <p className="import-status-line muted">{t('line.unread', { message: read.error.message })}</p>;
  }
  const counts = read.counts;
  if (place.mode === 'advanced') {
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
  return (
    <p className="import-status-line" aria-live="polite">
      <b>{counts.items === 1 ? t('line.itemsOne') : t('line.items', { count: number(counts.items) })}</b>{' '}
      {counts.left === 0 ? null : <>{t('line.left', { count: number(counts.left) })} </>}
      {counts.kinds === 0 ? (
        t('line.nothing')
      ) : (
        <a href={placeHref(id, { mode: 'simple', screen: 'check' })}>
          {counts.kinds === 1 ? t('line.lookOne') : t('line.look', { count: number(counts.kinds) })}
        </a>
      )}
      {counts.kinds === 0 ? ' ' : '. '}
      {counts.stops === 0 ? null : <>{t('line.stops', { count: number(counts.stops) })} </>}
      <span className="muted">{t('line.notYet')}</span>
    </p>
  );
};
