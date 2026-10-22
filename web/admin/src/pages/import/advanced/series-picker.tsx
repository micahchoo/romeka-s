/**
 * Which folder an Advanced screen of one series shows: a choice of the
 * plan's series by their folders' names, shown once the plan has several.
 * Picking one writes it to the address (`picked-series.ts`).
 */
import type { ReactElement } from 'react';
import type { Series } from '../../../api/import-plan';
import { useDraft } from '../draft-context';
import { pickedHref } from '../picked-series';
import { allSeries } from '../series';
import { whereWords } from '../words';
import { useAdvancedText } from './messages';
import { Setting } from './setting';

export const SeriesPicker = ({ picked }: { readonly picked: Series }): ReactElement | null => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const all = allSeries(draft.plan);
  if (all.length < 2) {
    return null;
  }
  const whole = whereWords(draft.plan.sources, t);
  const nameOf = (series: Series): string =>
    series.scope === 'all' ? whole : series.scope.map((scope) => (scope.folder === '' ? whole : scope.folder)).join(', ');
  return (
    <Setting label={t('adv.series.label')} plain={t('adv.series.plain')}>
      {(described) => (
        <select
          aria-label={t('adv.series.label')}
          aria-describedby={described}
          value={picked.id}
          onChange={(event) => {
            window.location.hash = pickedHref(draft.id, draft.place, event.target.value);
          }}
        >
          {all.map((series) => (
            <option key={series.id} value={series.id}>
              {nameOf(series)}
            </option>
          ))}
        </select>
      )}
    </Setting>
  );
};

/** What an Advanced screen of one series says when the plan imports no folder. */
export const NoSeries = (): ReactElement => {
  const { t } = useAdvancedText();
  return <p className="import-note">{t('adv.series.none')}</p>;
};
