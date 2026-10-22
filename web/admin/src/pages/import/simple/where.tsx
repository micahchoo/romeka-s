/**
 * "Where should they go in Omeka?" as two dials. Item sets: one segmented
 * choice (per folder, per top folder, one new set named in place, one already
 * in Omeka, a list's column, none) and what it makes, each set with its
 * items. Item types: one select for where they come from, then a compact
 * table of each value and its item type. A value is set to an item type only
 * when an item type in Omeka has the same name; every other value says it is
 * not chosen, and the person maps it (the item type rule's value map).
 *
 * The values are a table, not a list beside an inspector: each value's whole
 * decision is one select, so an inspector would add a click and show nothing
 * more.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { LevelId } from '../../../api/import-plan';
import type { ResourceTemplate } from '../../../api/types';
import { ErrorNotice } from '../../../app/errors';
import { Pager } from '../../../app/list';
import { TableScroll } from '../../../app/table-scroll';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadCatalog } from '../lists/catalog';
import type { Catalog } from '../lists/catalog';
import { usePreview } from '../lists/use-preview';
import { PER_PAGE } from '../paged-list';
import { fold } from './list-plan';
import { Segmented } from './master-detail';
import type { Segment } from './master-detail';
import { useSimpleText } from './messages';
import { folderName, setChoiceOf, setColumnOf, typeAnswerOf, typeFromOf, withSetChoice, withTypeAnswer, withTypeFrom, withoutTypeMap } from './where-plan';
import type { SetChoice, TypeFrom } from './where-plan';
import { usePickedSeries } from '../picked-node';
import { seriesOf } from '../series';
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
    </section>
  );
};

const fromKey = (from: TypeFrom): string => (from.from === 'column' ? `column:${from.sheet}:${String(from.index)}` : from.from);

const TypeValues = ({ level, templates }: { readonly level: LevelId; readonly templates: readonly ResourceTemplate[] }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [page, setPage] = useState(1);
  const read = usePreview(['values'], { plan: withoutTypeMap(draft.plan, level), values: { page, perPage: PER_PAGE } });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const values = read.data.values.find((one) => one.slot === 'template')?.values;
  if (values === undefined || values.total === 0) {
    return <p className="muted">{t('guided.where.typesEmpty')}</p>;
  }
  const lastPage = Math.max(1, Math.ceil(values.total / values.perPage));
  const newLabels = draft.plan.newTypes.map((type) => type.label);
  return (
    <>
      <TableScroll label={t('guided.where.typesTable')}>
        <table className="guided-columns" aria-busy={read.busy}>
          <thead>
            <tr>
              <th scope="col">{values.total === 1 ? t('guided.where.valuesOne') : t('guided.where.values', { count: number(values.total) })}</th>
              <th scope="col" className="num">{t('guided.where.colItems')}</th>
              <th scope="col">{t('guided.where.colType')}</th>
            </tr>
          </thead>
          <tbody>
            {values.items.map((value) => {
              const same = templates.find((template) => fold(template.label) === fold(value.text));
              const answer = typeAnswerOf(draft.plan, level, value.text);
              const chosen =
                answer.to === 'none' ? 'none' : answer.to === 'type' ? (newLabels.some((label) => fold(label) === fold(answer.label)) ? `new:${answer.label}` : `type:${answer.label}`) : same === undefined ? '' : `type:${same.label}`;
              return (
                <tr key={value.text} data-decision={`type:${fold(value.text)}`}>
                  <th scope="row">
                    {value.spellings.slice(0, 4).join(', ')}
                    {value.spellings.length > 1 ? <div className="muted">{t('guided.where.spellings', { count: number(value.spellings.length) })}</div> : null}
                  </th>
                  <td className="num">{number(value.count)}</td>
                  <td>
                    <select
                      aria-label={t('guided.where.typeFor', { value: value.text })}
                      value={chosen}
                      disabled={!draft.editable}
                      onChange={(event) => {
                        const next = event.target.value;
                        draft.change((plan) =>
                          next === ''
                            ? withTypeAnswer(plan, level, value.text, { to: 'unset' }, false)
                            : next === 'none'
                              ? withTypeAnswer(plan, level, value.text, { to: 'none' }, false)
                              : next.startsWith('new:')
                                ? withTypeAnswer(plan, level, value.text, { to: 'type', label: next.slice('new:'.length) }, true)
                                : withTypeAnswer(plan, level, value.text, { to: 'type', label: next.slice('type:'.length) }, false),
                        );
                      }}
                    >
                      <option value="">{t('guided.where.notChosen')}</option>
                      {templates.map((template) => (
                        <option key={template.id} value={`type:${template.label}`}>
                          {template === same ? t('guided.where.sameName', { type: template.label }) : template.label}
                        </option>
                      ))}
                      <option value={`new:${value.text}`}>{t('guided.where.newType', { type: value.text })}</option>
                      <option value="none">{t('guided.where.noType')}</option>
                    </select>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableScroll>
      {lastPage > 1 ? <Pager page={page} lastPage={lastPage} total={values.total} label={t('guided.where.valuesResource')} onPage={setPage} /> : null}
    </>
  );
};

const ItemTypes = ({ level, catalog }: { readonly level: LevelId; readonly catalog: Catalog | null }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const from = typeFromOf(draft.plan, level);
  const { lists } = seriesOf(draft.plan, level);
  const columns = lists.flatMap((sheet) => sheet.columns.map((column) => ({ sheet: sheet.id, file: sheet.file, column: column.column })));
  const several = lists.length > 1;
  return (
    <section className="guided-inspector guided-inspect" aria-label={t('guided.where.typesTitle')}>
      <h4>{t('guided.where.typesTitle')}</h4>
      <dl className="guided-kv">
        <dt>{t('guided.where.from')}</dt>
        <dd data-decision="type-from">
          <select
            aria-label={t('guided.where.typeFrom')}
            value={fromKey(from)}
            disabled={!draft.editable}
            onChange={(event) => {
              const value = event.target.value;
              const column = columns.find((one) => `column:${one.sheet}:${String(one.column.index)}` === value);
              const next: TypeFrom = column !== undefined ? { from: 'column', sheet: column.sheet, index: column.column.index } : value === 'top' ? { from: 'top' } : { from: 'none' };
              draft.change((plan) => withTypeFrom(plan, level, next));
            }}
          >
            <option value="none">{t('guided.where.fromNone')}</option>
            {columns.map((one) => (
              <option key={`${one.sheet}:${String(one.column.index)}`} value={`column:${one.sheet}:${String(one.column.index)}`}>
                {several ? t('guided.where.fromColumnOf', { column: one.column.header, file: one.file }) : t('guided.where.fromColumn', { column: one.column.header })}
              </option>
            ))}
            <option value="top">{t('guided.where.fromTop')}</option>
          </select>
        </dd>
      </dl>
      {from.from === 'none' ? null : <TypeValues level={level} templates={catalog?.templates ?? []} />}
    </section>
  );
};

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
      <ItemTypes level={series.id} catalog={ready} />
      <Help />
    </div>
  );
};

export default WhereTheyGo;
