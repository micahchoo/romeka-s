/**
 * Files that are byte for byte the same as another file: equal size, then an
 * equal SHA-256 (`identical`). The list is facts only; the answer to them is
 * the plan's `anomalies.identical`, which Check and Problems ask.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { ErrorNotice } from '../../../app/errors';
import { Pager } from '../../../app/list';
import { useImportText } from '../messages';
import { usePreview } from './use-preview';

export const IDENTICAL_PER_PAGE = 20;

export const IdenticalFiles = (): ReactElement | null => {
  const { t, number } = useImportText();
  const [page, setPage] = useState(1);
  const read = usePreview(['identical'], { identical: { page, perPage: IDENTICAL_PER_PAGE } });
  if (read.status === 'loading') {
    return null;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { identical } = read.data;
  if (identical.files === 0) {
    return null;
  }
  const lastPage = Math.max(1, Math.ceil(identical.sets.total / identical.sets.perPage));
  return (
    <section className="import-identical" aria-busy={read.busy} aria-label={t('lists.identical.label')}>
      <p>
        <b>{identical.files === 1 ? t('lists.identical.countOne') : t('lists.identical.count', { count: number(identical.files) })}</b>{' '}
        {t('lists.identical.why')}
      </p>
      <ul>
        {identical.sets.items.map((set) => (
          <li key={`${set.source}/${set.sha256}`}>
            {set.paths.join(', ')} <span className="muted">{t('lists.identical.size', { bytes: number(set.size) })}</span>
          </li>
        ))}
      </ul>
      {lastPage > 1 ? <Pager page={page} lastPage={lastPage} total={identical.sets.total} label={t('lists.identical.resource')} onPage={setPage} /> : null}
    </section>
  );
};

export default IdenticalFiles;
