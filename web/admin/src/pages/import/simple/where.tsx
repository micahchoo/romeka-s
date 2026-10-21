/**
 * "Where should they go in Omeka?": the item set each item goes into, from the
 * folders or one for everything, and the item type of each, from nowhere, a
 * list's column or the top folder. A value is set to an item type only when
 * an item type in Omeka has the same name; every other value says it is not
 * chosen, and the person maps it (the item type rule's value map). Values
 * written differently are different values: the screen says so.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
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
import { useSimpleText } from './messages';
import { folderName, setChoiceOf, setColumnOf, typeAnswerOf, typeFromOf, withSets, withTypeAnswer, withTypeFrom, withoutTypeMap } from './where-plan';
import type { SetChoice, TypeFrom } from './where-plan';
import './simple.css';

const CHIPS_SHOWN = 16;

const ItemSets = ({ catalog }: { readonly catalog: Catalog | null }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const read = usePreview(['values'], { values: { perPage: CHIPS_SHOWN } });
  const current = setChoiceOf(draft.plan);
  const name = folderName(draft.plan);
  const column = setColumnOf(draft.plan);
  const existing = catalog?.itemSets ?? [];
  const choices: readonly { readonly id: SetChoice; readonly label: string }[] = [
    { id: 'leaf', label: t('guided.where.setLeaf') },
    { id: 'top', label: t('guided.where.setTop') },
    { id: 'one', label: t('guided.where.setOne', { name }) },
    ...(existing.length === 0 ? [] : [{ id: 'old' as const, label: t('guided.where.setOld') }]),
    ...(column === null ? [] : [{ id: 'column' as const, label: t('guided.where.setColumn', { header: column }) }]),
    { id: 'none', label: t('guided.where.setNone') },
  ];
  const sets = read.status === 'ready' ? read.data.values.find((one) => one.slot === 'sets')?.values : undefined;
  return (
    <>
      <h4>{t('guided.where.setsTitle')}</h4>
      <p>
        {t('guided.where.setsAsk')} <span className="muted">{t('guided.where.setsGloss')}</span>
      </p>
      <fieldset className="guided-opts" data-decision="sets" aria-label={t('guided.where.setsTitle')}>
        {choices.map((choice) => (
          <label key={choice.id}>
            <input
              type="radio"
              name="item-sets"
              checked={current.choice === choice.id}
              disabled={!draft.editable}
              onChange={() => {
                const first = existing[0];
                draft.change((plan) => withSets(plan, choice.id, choice.id === 'old' ? (first === undefined ? null : String(first.id)) : null));
              }}
            />
            <span>{choice.label}</span>
          </label>
        ))}
      </fieldset>
      {current.choice === 'old' ? (
        <label>
          {t('guided.where.setWhich')}{' '}
          <select
            value={current.set ?? ''}
            disabled={!draft.editable}
            onChange={(event) => {
              const set = event.target.value;
              draft.change((plan) => withSets(plan, 'old', set));
            }}
          >
            {existing.slice(0, 200).map((set) => (
              <option key={set.id} value={String(set.id)}>
                {set.title ?? `#${String(set.id)}`}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {read.status === 'error' ? <ErrorNotice error={read.error} /> : null}
      {sets === undefined ? null : (
        <p aria-busy={read.status === 'ready' && read.busy}>
          {sets.total === 0 ? t('guided.where.setsNone') : sets.total === 1 ? t('guided.where.setsMakeOne') : t('guided.where.setsMake', { count: number(sets.total) })}{' '}
          {sets.items.map((set) => (
            <span key={set.text} className="import-chip">
              {set.text} · {number(set.count)}
            </span>
          ))}
          {sets.total > sets.items.length ? <span className="muted"> {t('guided.where.andMore', { count: number(sets.total - sets.items.length) })}</span> : null}
        </p>
      )}
    </>
  );
};

const fromKey = (from: TypeFrom): string => (from.from === 'column' ? `column:${from.sheet}:${String(from.index)}` : from.from);

const TypeValues = ({ templates }: { readonly templates: readonly ResourceTemplate[] }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [page, setPage] = useState(1);
  const read = usePreview(['values'], { plan: withoutTypeMap(draft.plan), values: { page, perPage: PER_PAGE } });
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
      <p>{values.total === 1 ? t('guided.where.valuesOne') : t('guided.where.values', { count: number(values.total) })} {t('guided.where.valuesNote')}</p>
      <TableScroll label={t('guided.where.typesTable')}>
        <table aria-busy={read.busy}>
          <thead>
            <tr>
              <th scope="col">{t('guided.where.colValue')}</th>
              <th scope="col" className="num">{t('guided.where.colItems')}</th>
              <th scope="col">{t('guided.where.colType')}</th>
            </tr>
          </thead>
          <tbody>
            {values.items.map((value) => {
              const same = templates.find((template) => fold(template.label) === fold(value.text));
              const answer = typeAnswerOf(draft.plan, value.text);
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
                            ? withTypeAnswer(plan, value.text, { to: 'unset' }, false)
                            : next === 'none'
                              ? withTypeAnswer(plan, value.text, { to: 'none' }, false)
                              : next.startsWith('new:')
                                ? withTypeAnswer(plan, value.text, { to: 'type', label: next.slice('new:'.length) }, true)
                                : withTypeAnswer(plan, value.text, { to: 'type', label: next.slice('type:'.length) }, false),
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

const ItemTypes = ({ catalog }: { readonly catalog: Catalog | null }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const from = typeFromOf(draft.plan);
  const columns = draft.plan.sheets.flatMap((sheet) => sheet.columns.map((column) => ({ sheet: sheet.id, column: column.column })));
  return (
    <>
      <h4>{t('guided.where.typesTitle')}</h4>
      <p>
        {t('guided.where.typesAsk')} <span className="muted">{t('guided.where.typesGloss')}</span>
      </p>
      <label data-decision="type-from">
        {t('guided.where.typeFrom')}{' '}
        <select
          value={fromKey(from)}
          disabled={!draft.editable}
          onChange={(event) => {
            const value = event.target.value;
            const column = columns.find((one) => `column:${one.sheet}:${String(one.column.index)}` === value);
            const next: TypeFrom = column !== undefined ? { from: 'column', sheet: column.sheet, index: column.column.index } : value === 'top' ? { from: 'top' } : { from: 'none' };
            draft.change((plan) => withTypeFrom(plan, next));
          }}
        >
          <option value="none">{t('guided.where.fromNone')}</option>
          {columns.map((one) => (
            <option key={`${one.sheet}:${String(one.column.index)}`} value={`column:${one.sheet}:${String(one.column.index)}`}>
              {t('guided.where.fromColumn', { column: one.column.header })}
            </option>
          ))}
          <option value="top">{t('guided.where.fromTop')}</option>
        </select>
      </label>
      {from.from === 'none' ? null : <TypeValues templates={catalog?.templates ?? []} />}
    </>
  );
};

export const WhereTheyGo = (): ReactElement => {
  const catalog = useAsync(loadCatalog, []);
  const ready = catalog.status === 'ready' ? catalog.data : null;
  return (
    <>
      {catalog.status === 'error' ? <ErrorNotice error={catalog.error} /> : null}
      <ItemSets catalog={ready} />
      <hr />
      <ItemTypes catalog={ready} />
    </>
  );
};

export default WhereTheyGo;
