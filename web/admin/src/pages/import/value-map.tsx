/**
 * The value map control (P6 design, section 2), one for every place a value
 * can become another: the item type from a column, one set per value,
 * "Clean values" on a text column, and every Advanced rule card. A row is one
 * value as the server counts it before the map, with its other spellings,
 * the records that hold it, and what it becomes. Near spellings show above
 * the table, each with its counts and one button; nothing joins without
 * that click. It loads as a chunk of its own.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { ResourceTemplate, ItemSet } from '../../api/types';
import type { NearSpelling, ValueCount } from '../../api/import-sections';
import { ErrorNotice } from '../../app/errors';
import { Pager } from '../../app/list';
import { TableScroll } from '../../app/table-scroll';
import { useAsync } from '../../app/useAsync';
import { useDraft } from './draft-context';
import { loadCatalog } from './lists/catalog';
import { usePreview } from './lists/use-preview';
import { PER_PAGE } from './paged-list';
import { withOnlyFreeSet } from './series';
import { fold } from './simple/list-plan';
import { useSimpleText } from './simple/messages';
import type { SimpleTranslator } from './simple/messages';
import { becomesOf, mapOf, mappedOf, slotOf, withEntry, withNewType } from './value-map-edits';
import type { Becomes, MapHome } from './value-map-edits';

/** The most near spellings shown at once: each is one line the person may ignore. */
const NEAR_SHOWN = 5;

const KEEP = '';
const OUT = 'none';
const TYPED = 'typed';

interface Choice {
  readonly value: string;
  readonly label: string;
}

/** What a value may become, besides as written and left out, for the slot's kind of output. */
const choicesFor = (becomes: Becomes, value: ValueCount, values: readonly ValueCount[], install: { readonly templates: readonly ResourceTemplate[]; readonly itemSets: readonly ItemSet[] }, newTypes: readonly string[], t: SimpleTranslator['t']): readonly Choice[] => {
  switch (becomes) {
    case 'type':
      return [
        ...install.templates.map((template) => ({ value: `to:${template.label}`, label: fold(template.label) === fold(value.text) ? t('guided.where.sameName', { type: template.label }) : template.label })),
        ...newTypes.filter((label) => !install.templates.some((template) => fold(template.label) === fold(label))).map((label) => ({ value: `to:${label}`, label })),
        ...(newTypes.some((label) => fold(label) === fold(value.text)) ? [] : [{ value: `new:${value.text}`, label: t('guided.where.newType', { type: value.text }) }]),
      ];
    case 'set':
      return [
        ...install.itemSets.flatMap((set) => (set.title === null ? [] : [{ value: `to:${set.title}`, label: t('valueMap.inOmeka', { title: set.title }) }])),
        { value: TYPED, label: t('valueMap.typed') },
      ];
    case 'class':
      return [{ value: TYPED, label: t('valueMap.typed') }];
    case 'text':
      return [...values.filter((other) => fold(other.text) !== fold(value.text)).map((other) => ({ value: `to:${other.text}`, label: t('valueMap.sameAs', { value: other.text }) })), { value: TYPED, label: t('valueMap.typed') }];
  }
};

const keepLabel = (becomes: Becomes, value: ValueCount, templates: readonly ResourceTemplate[], t: SimpleTranslator['t']): string => {
  if (becomes !== 'type') {
    return t('valueMap.asWritten');
  }
  const same = templates.find((template) => fold(template.label) === fold(value.text));
  return same === undefined ? t('guided.where.notChosen') : t('guided.where.sameName', { type: same.label });
};

const outLabel = (becomes: Becomes, t: SimpleTranslator['t']): string => (becomes === 'type' ? t('guided.where.noType') : becomes === 'set' ? t('valueMap.noSet') : t('valueMap.leaveOut'));

/** One value's "becomes": a choice, and a text box when the text is typed. A blank text is never saved. */
const BecomesChoice = ({ home, becomes, value, values, install }: { readonly home: MapHome; readonly becomes: Becomes; readonly value: ValueCount; readonly values: readonly ValueCount[]; readonly install: { readonly templates: readonly ResourceTemplate[]; readonly itemSets: readonly ItemSet[] } }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const [typing, setTyping] = useState(false);
  const to = mappedOf(mapOf(draft.plan, home), value.text);
  const choices = choicesFor(becomes, value, values, install, draft.plan.newTypes.map((type) => type.label), t);
  const offered = to === undefined ? KEEP : to === '' ? OUT : choices.some((choice) => choice.value === `to:${to}`) ? `to:${to}` : TYPED;
  const chosen = typing ? TYPED : offered;
  const typedAllowed = choices.some((choice) => choice.value === TYPED);
  return (
    <>
      <select
        aria-label={becomes === 'type' ? t('guided.where.typeFor', { value: value.text }) : t('valueMap.becomesOf', { value: value.text })}
        value={chosen}
        disabled={!draft.editable}
        onChange={(event) => {
          const next = event.target.value;
          setTyping(next === TYPED);
          if (next === TYPED) {
            return;
          }
          draft.change((plan) => {
            if (next.startsWith('new:')) {
              const label = next.slice('new:'.length);
              return withNewType(withEntry(plan, home, value.text, label), label);
            }
            return withEntry(plan, home, value.text, next === KEEP ? undefined : next === OUT ? '' : next.slice('to:'.length));
          });
        }}
      >
        <option value={KEEP}>{keepLabel(becomes, value, install.templates, t)}</option>
        {choices.map((choice) => (
          <option key={choice.value} value={choice.value}>
            {choice.label}
          </option>
        ))}
        {offered === TYPED && !typedAllowed && to !== undefined ? <option value={TYPED}>{to}</option> : null}
        <option value={OUT}>{outLabel(becomes, t)}</option>
      </select>
      {chosen === TYPED && typedAllowed ? (
        <input
          type="text"
          aria-label={t('valueMap.typedOf', { value: value.text })}
          defaultValue={to ?? ''}
          disabled={!draft.editable}
          onChange={(event) => {
            const text = event.target.value.trim();
            if (text !== '') {
              draft.change((plan) => withEntry(plan, home, value.text, text));
            }
          }}
        />
      ) : null}
    </>
  );
};

const NearLine = ({ home, near, counted }: { readonly home: MapHome; readonly near: NearSpelling; readonly counted: (count: number) => string }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  return (
    <li>
      {t('valueMap.near', { from: near.from, fromCount: counted(near.fromRecords), to: near.to, toCount: counted(near.toRecords) })}{' '}
      <button type="button" disabled={!draft.editable} onClick={() => { draft.change((plan) => withEntry(plan, home, near.from, near.to)); }}>
        {t('valueMap.nearUse', { to: near.to })}
      </button>
    </li>
  );
};

export interface ValueMapProps {
  readonly home: MapHome;
  /** The table's name. */
  readonly label: string;
}

export const ValueMap = ({ home, label }: ValueMapProps): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [page, setPage] = useState(1);
  const slot = slotOf(home);
  const becomes = becomesOf(slot);
  const catalog = useAsync(() => (becomes === 'type' || becomes === 'set' ? loadCatalog() : Promise.resolve({ templates: [], itemSets: [] })), [becomes]);
  const install = catalog.status === 'ready' ? catalog.data : { templates: [], itemSets: [] };
  const read = usePreview(
    ['values'],
    'freeSet' in home
      ? { plan: withOnlyFreeSet(draft.plan, home.freeSet), values: { slot, kind: 'item', page, perPage: PER_PAGE } }
      : { values: { slot, kind: home.kind, series: home.level, page, perPage: PER_PAGE } },
  );
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const found = read.data.values.find((one) => one.slot === slot);
  if (found === undefined || found.values.total === 0) {
    return <p className="muted">{t('valueMap.none')}</p>;
  }
  const { values, near } = found;
  const files = 'freeSet' in home ? false : home.kind === 'media';
  const counted = (count: number): string =>
    files ? (count === 1 ? t('valueMap.fileOne') : t('guided.list.levelFiles', { count: number(count) })) : count === 1 ? t('valueMap.itemOne') : t('guided.list.levelItems', { count: number(count) });
  const lastPage = Math.max(1, Math.ceil(values.total / values.perPage));
  return (
    <div className="value-map" aria-busy={read.busy}>
      {catalog.status === 'error' ? <ErrorNotice error={catalog.error} /> : null}
      {near.length === 0 ? null : (
        <ul className="value-map-near">
          {near.slice(0, NEAR_SHOWN).map((one) => (
            <NearLine key={`${one.from}→${one.to}`} home={home} near={one} counted={counted} />
          ))}
        </ul>
      )}
      <TableScroll label={label}>
        <table className="guided-columns">
          <thead>
            <tr>
              <th scope="col">{values.total === 1 ? t('guided.where.valuesOne') : t('guided.where.values', { count: number(values.total) })}</th>
              <th scope="col" className="num">{files ? t('valueMap.colFiles') : t('guided.where.colItems')}</th>
              <th scope="col">{becomes === 'type' ? t('guided.where.colType') : t('valueMap.becomes')}</th>
            </tr>
          </thead>
          <tbody>
            {values.items.map((value) => (
              <tr key={value.text} data-decision={`map:${fold(value.text)}`}>
                <th scope="row">
                  {value.spellings.slice(0, 4).join(', ')}
                  {value.spellings.length > 1 ? <div className="muted">{t('guided.where.spellings', { count: number(value.spellings.length) })}</div> : null}
                </th>
                <td className="num">{number(value.count)}</td>
                <td>
                  <BecomesChoice home={home} becomes={becomes} value={value} values={values.items} install={install} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </TableScroll>
      {lastPage > 1 ? <Pager page={page} lastPage={lastPage} total={values.total} label={t('guided.where.valuesResource')} onPage={setPage} /> : null}
    </div>
  );
};

export default ValueMap;
