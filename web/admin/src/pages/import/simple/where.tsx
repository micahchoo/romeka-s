/**
 * "Where should they go in Omeka?" as two dials. Item sets: one segmented
 * choice (per folder, per top folder, one new set named in place, one already
 * in Omeka, a list's column, none) and what it makes, each set with its
 * items; for a list's column, what each value's set becomes. Item types: one
 * select for where they come from, then each value and its item type. A
 * value is set to an item type only when an item type in Omeka has the same
 * name; every other value says it is not chosen, and the person maps it.
 * Both tables are the value map control (`../value-map.tsx`); the item
 * types are Structure's own (`item-types.tsx`).
 */
import { Suspense, lazy, useState } from 'react';
import type { ReactElement } from 'react';
import { ErrorNotice } from '../../../app/errors';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadCatalog } from '../lists/catalog';
import type { Catalog } from '../lists/catalog';
import { usePreview } from '../lists/use-preview';
import { Segmented } from './master-detail';
import type { Segment } from './master-detail';
import { useSimpleText } from './messages';
import { folderName, setChoiceOf, setColumnOf, withSetChoice } from './where-plan';
import type { SetChoice } from './where-plan';
import { usePickedSeries } from '../picked-node';
import { setsOf } from '../series';
import './simple.css';

const CHIPS_SHOWN = 16;

/** What the item set choice makes: the count, then each set with its items. */
const SetsMade = (): ReactElement | null => {
  const { t, number } = useSimpleText();
  const read = usePreview(['values'], { values: { perPage: CHIPS_SHOWN } });
  const sets = read.status === 'ready' ? read.data.values.find((one) => one.slot === 'sets')?.values : undefined;
  if (read.status === 'error') {
    return (
      <>
        <dt>{t('guided.where.makes')}</dt>
        <dd>
          <ErrorNotice error={read.error} />
        </dd>
      </>
    );
  }
  if (read.status !== 'ready' || sets === undefined) {
    return null;
  }
  return (
    <>
      <dt>{t('guided.where.makes')}</dt>
      <dd aria-busy={read.busy}>
        {sets.total === 0 ? t('guided.where.setsNone') : sets.total === 1 ? t('guided.where.setsMakeOne') : t('guided.where.setsMake', { count: number(sets.total) })}{' '}
        {sets.items.map((set) => (
          <span key={set.text} className="guided-chip">
            {set.text} · {number(set.count)}
          </span>
        ))}
        {sets.total > sets.items.length ? <span className="muted">{t('guided.where.andMore', { count: number(sets.total - sets.items.length) })}</span> : null}
      </dd>
    </>
  );
};

/**
 * The new set's name, saved as it is typed. A blank name is never saved: the
 * field keeps what was typed and the plan keeps the last name.
 */
const SetName = ({ saved }: { readonly saved: string }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const [typed, setTyped] = useState<string | null>(null);
  return (
    <input
      type="text"
      aria-label={t('guided.where.setName')}
      value={typed ?? saved}
      disabled={!draft.editable}
      onChange={(event) => {
        const text = event.target.value;
        setTyped(text);
        if (text.trim() !== '') {
          draft.change((plan) => withSetChoice(plan, 'one', text.trim()));
        }
      }}
    />
  );
};

const ItemSets = ({ catalog }: { readonly catalog: Catalog | null }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const current = setChoiceOf(draft.plan);
  const column = setColumnOf(draft.plan);
  const eachAt = setsOf(draft.plan).findIndex((set) => 'each' in set);
  const existing = catalog?.itemSets ?? [];
  const segment = (value: SetChoice, label: string): Segment<SetChoice> => ({ value, label, suggested: false });
  const choices: readonly Segment<SetChoice>[] = [
    segment('leaf', t('guided.where.setLeaf')),
    segment('top', t('guided.where.setTop')),
    segment('one', t('guided.where.setOne')),
    ...(existing.length === 0 ? [] : [segment('old', t('guided.where.setOld'))]),
    ...(column === null ? [] : [segment('column', t('guided.where.setColumn', { header: column }))]),
    segment('none', t('guided.where.setNone')),
  ];
  return (
    <section className="guided-inspector guided-inspect" aria-label={t('guided.where.setsTitle')}>
      <h4>{t('guided.where.setsTitle')}</h4>
      <dl className="guided-kv">
        <dt>{t('guided.where.from')}</dt>
        <dd data-decision="sets">
          <Segmented
            label={t('guided.where.setsTitle')}
            segments={choices}
            value={current.choice}
            disabled={!draft.editable}
            suggestedWord={t('guided.check.suggested')}
            onChange={(choice) => {
              const first = existing[0];
              draft.change((plan) => withSetChoice(plan, choice, choice === 'old' ? (first === undefined ? null : String(first.id)) : null));
            }}
          />
          {current.choice === 'one' ? <SetName saved={current.set ?? folderName(draft.plan)} /> : null}
          {current.choice === 'old' ? (
            <select
              aria-label={t('guided.where.setWhich')}
              value={current.set ?? ''}
              disabled={!draft.editable}
              onChange={(event) => {
                const set = event.target.value;
                draft.change((plan) => withSetChoice(plan, 'old', set));
              }}
            >
              {existing.slice(0, 200).map((set) => (
                <option key={set.id} value={String(set.id)}>
                  {set.title ?? `#${String(set.id)}`}
                </option>
              ))}
            </select>
          ) : null}
        </dd>
        <SetsMade />
      </dl>
      {current.choice === 'column' && eachAt >= 0 ? (
        <Suspense fallback={null}>
          <ValueMap home={{ freeSet: eachAt }} label={t('guided.where.setsTable')} />
        </Suspense>
      ) : null}
    </section>
  );
};

const ValueMap = lazy(() => import('../value-map'));
const ItemTypes = lazy(() => import('./item-types'));

const Help = (): ReactElement => {
  const { t } = useSimpleText();
  return (
    <details className="guided-help">
      <summary>{t('guided.list.help')}</summary>
      <p>{t('guided.where.setsGloss')}</p>
      <p>{t('guided.where.typesGloss')}</p>
      <p>{t('guided.where.valuesNote')}</p>
    </details>
  );
};

/** The plan's one series: this screen opens only on a plan of one (`screens.ts`, `landing`). */
export const WhereTheyGo = (): ReactElement | null => {
  const catalog = useAsync(loadCatalog, []);
  const ready = catalog.status === 'ready' ? catalog.data : null;
  const series = usePickedSeries();
  if (series === null) {
    return null;
  }
  return (
    <div className="guided-dials guided-stack">
      {catalog.status === 'error' ? <ErrorNotice error={catalog.error} /> : null}
      <ItemSets catalog={ready} />
      <Suspense fallback={null}>
        <ItemTypes level={series.id} />
      </Suspense>
      <Help />
    </div>
  );
};

export default WhereTheyGo;
