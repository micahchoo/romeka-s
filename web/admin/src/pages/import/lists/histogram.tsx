/**
 * Files in each item, as bars (`histogram`). A wrong grouping shows here
 * first: almost every item has one file, or one item has thousands. Each bar
 * opens its items in the items list.
 */
import type { ReactElement } from 'react';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { useListsText } from './messages';
import { listHref } from './address';
import { BUCKET_WORDS } from './facts-words';
import { usePreview } from './use-preview';

export const FilesChart = (): ReactElement | null => {
  const { t, number } = useListsText();
  const draft = useDraft();
  const read = usePreview(['histogram'], {});
  if (read.status === 'loading') {
    return null;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const bars = read.data.histogram;
  const most = Math.max(1, ...bars.map((bar) => bar.count));
  const items = draft.place.mode === 'simple' ? ({ mode: 'simple', screen: 'all-items' } as const) : ({ mode: 'advanced', screen: 'items' } as const);
  return (
    <figure className="import-chart" aria-busy={read.busy}>
      <figcaption>{t('lists.chart.caption')}</figcaption>
      <ul>
        {bars.map((bar) => (
          <li key={bar.files}>
            <a
              href={listHref(draft.id, items, { files: bar.files })}
              aria-label={t('lists.chart.bar', { count: number(bar.count), files: t(BUCKET_WORDS[bar.files]) })}
            >
              <span className="import-chart-label">{t(BUCKET_WORDS[bar.files])}</span>
              <span className="import-chart-track" aria-hidden="true">
                <span className="import-chart-fill" style={{ width: bar.count === 0 ? 0 : `${String(Math.max(1, (bar.count / most) * 100))}%` }} />
              </span>
              <span className="import-chart-count">{number(bar.count)}</span>
            </a>
          </li>
        ))}
      </ul>
    </figure>
  );
};

export default FilesChart;
