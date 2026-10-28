/**
 * The dashboard's "Needs attention" list: each line a count of items that wait for work, linking to
 * the Items list it counts, where every Items tool works on them. The server reads each count
 * through that list's own matching, so a count is the list's total. A count of 0 stays a plain line
 * with no link, so the list keeps its shape and an empty list is never one click away. Until the
 * counts arrive the list says it is counting, and never shows a number it has not read.
 */
import type { ReactElement } from 'react';
import { dashboardApi } from '../../api/dashboard';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { admin } from '../../i18n/namespaces/admin';
import { itemsHref, NONE } from '../items/browse';
import { FixityLine } from './fixity-line';

interface Line {
  readonly key: string;
  readonly text: string;
  readonly count: number;
  readonly href: string;
}

const CountLine = ({ line }: { readonly line: Line }): ReactElement => (
  <li>{line.count === 0 ? line.text : <a href={line.href}>{line.text}</a>}</li>
);

export const NeedsAttention = (): ReactElement => {
  const { t, number } = useTranslator(admin);
  const state = useAsync(() => dashboardApi.counts(), []);
  let body: ReactElement;
  if (state.status === 'loading') {
    body = <p role="status">{t('dashboard.counting')}</p>;
  } else if (state.status === 'error') {
    body = <ErrorNotice error={state.error} />;
  } else {
    const counts = state.data;
    const lines: readonly Line[] = [
      {
        key: 'set',
        text: t('dashboard.noItemSet', { count: number(counts.noItemSet) }),
        count: counts.noItemSet,
        href: itemsHref({ itemSetId: NONE }),
      },
      {
        key: 'type',
        text: t('dashboard.noItemType', { count: number(counts.noItemType) }),
        count: counts.noItemType,
        href: itemsHref({ templateId: NONE }),
      },
      {
        key: 'changed',
        text: t('dashboard.changedByOthers', { count: number(counts.changedByOthers.items) }),
        count: counts.changedByOthers.items,
        href: itemsHref({ changedSince: counts.changedByOthers.since }),
      },
    ];
    body = (
      <ul>
        {lines.map((line) => (
          <CountLine key={line.key} line={line} />
        ))}
      </ul>
    );
  }
  return (
    <section aria-labelledby="needs-attention-title">
      <h2 id="needs-attention-title">{t('dashboard.needsAttention')}</h2>
      {body}
      <FixityLine />
    </section>
  );
};
