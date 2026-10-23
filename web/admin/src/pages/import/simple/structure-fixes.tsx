/**
 * The corrections the proposal offers on a folder (`Fix`): "these files are
 * one item" under Items, and "this row is that file" and "this row describes
 * nothing" under List. Each offer is one row with its count and what the
 * click changes; "Show the N" lists each instance as facts, ticked. One
 * click sends the ticked instances as one edit, and an untick is one more
 * click. One question holds one edit: a later offer of the same kind on the
 * same folder or list adds its instances to the edit there. The server
 * treats corrections as cumulative, so this merge is the screen's rule.
 *
 * The changes strip lists the same instances under their edit, each with its
 * tick, and the ones that no longer hold with why. Unticking one there takes
 * it out of the edit; unticking the last takes the edit back.
 *
 * A lazy chunk: most folders have no offer.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { Because, EditResult, Fix, SeriesNode, StaleWhy } from '../../../api/import-proposal';
import type { Edit, FolderScope } from '../../../api/import-plan';
import { canonical, heldBy, questionOf } from './structure-edits';
import type { Fixing } from './structure-edits';
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { importFixes } from '../../../i18n/namespaces/import-fixes';
import { Notes, Row, useScreen } from './structure-parts';

type FixKey = KeyOf<typeof importFixes>;
type Words = Translator<MessageKey | FixKey>;
type T = Words['t'];

/** The words of the corrections, which load with this chunk. */
export const useFixText = (): Words => useTranslator(importFixes);

/** A count's words: the plural key, else the key for exactly one. */
type Counted = readonly [many: FixKey, one: FixKey];

interface KindWords {
  readonly line: Counted;
  readonly note: Counted | null;
  readonly take: Counted;
}

const KINDS: Readonly<Record<Fixing['edit'], KindWords>> = {
  oneItem: { line: ['fix.oneItem', 'fix.oneItemOne'], note: null, take: ['fix.oneItem.take', 'fix.oneItem.takeOne'] },
  rowIs: {
    line: ['fix.rowIs', 'fix.rowIsOne'],
    note: ['fix.rowIs.note', 'fix.rowIs.noteOne'],
    take: ['fix.rowIs.take', 'fix.rowIs.takeOne'],
  },
  describesNothing: {
    line: ['fix.describesNothing', 'fix.describesNothingOne'],
    note: ['fix.describesNothing.note', 'fix.describesNothing.noteOne'],
    take: ['fix.describesNothing.take', 'fix.describesNothing.takeOne'],
  },
};

const BECAUSE: Readonly<Record<Because, FixKey>> = {
  keyExtends: 'fix.because.keyExtends',
  rowNamesBoth: 'fix.because.rowNamesBoth',
  near: 'fix.because.near',
  namesItem: 'fix.because.namesItem',
  namesPart: 'fix.because.namesPart',
};

const STALE: Readonly<Record<StaleWhy, FixKey>> = {
  rowChanged: 'fix.rowChanged',
  fileGone: 'fix.fileGone',
};

const counted = ([many, one]: Counted, count: number, words: Words): string => (count === 1 ? words.t(one) : words.t(many, { count: words.number(count) }));

/** Each instance of a correction in words, in its order. */
const instanceWords = (edit: Fixing, t: T): readonly string[] => {
  switch (edit.edit) {
    case 'oneItem':
      return edit.joins.map((join) => t('fix.join', { with: join.with, item: join.item }));
    case 'rowIs':
      return edit.pairs.map((pair) => t('fix.pair', { line: String(pair.row.line), cell: pair.row.cell, file: pair.file }));
    case 'describesNothing':
      return edit.rows.map((row) => t('fix.leave', { line: String(row.line), cell: row.cell }));
  }
};

export const isFixing = (edit: Edit): edit is Fixing => edit.edit === 'oneItem' || edit.edit === 'rowIs' || edit.edit === 'describesNothing';

export const countOf = (edit: Fixing): number => (edit.edit === 'oneItem' ? edit.joins.length : edit.edit === 'rowIs' ? edit.pairs.length : edit.rows.length);

/** The correction with only the instances `keep` holds; `null` when none is left, which takes the edit back. */
const keeping = (edit: Fixing, keep: (at: number) => boolean): Fixing | null => {
  const left = <I,>(all: readonly I[]): readonly I[] => all.filter((_one, at) => keep(at));
  const next: Fixing =
    edit.edit === 'oneItem' ? { ...edit, joins: left(edit.joins) } : edit.edit === 'rowIs' ? { ...edit, pairs: left(edit.pairs) } : { ...edit, rows: left(edit.rows) };
  return countOf(next) === 0 ? null : next;
};

/** The edit that holds the question already, with `taken`'s instances after its own, each once. */
const joined = (held: Fixing | null, taken: Fixing): Fixing => {
  const more = <I,>(mine: readonly I[], theirs: readonly I[]): readonly I[] => [...mine, ...theirs.filter((one) => !mine.some((had) => canonical(had) === canonical(one)))];
  if (held?.edit === 'oneItem' && taken.edit === 'oneItem') {
    return { ...taken, joins: more(held.joins, taken.joins) };
  }
  if (held?.edit === 'rowIs' && taken.edit === 'rowIs') {
    return { ...taken, pairs: more(held.pairs, taken.pairs) };
  }
  if (held?.edit === 'describesNothing' && taken.edit === 'describesNothing') {
    return { ...taken, rows: more(held.rows, taken.rows) };
  }
  return taken;
};

/** The edit that takes every instance of an offer, as the server's `Fix::edit` makes it, with why each is offered. */
const offerOf = (fix: Fix, scope: FolderScope): { readonly edit: Fixing; readonly because: readonly (Because | null)[] } => {
  switch (fix.fix) {
    case 'oneItem':
      return { edit: { edit: 'oneItem', scope, joins: fix.joins.map((one) => one.join) }, because: fix.joins.map((one) => one.because) };
    case 'rowIs':
      return { edit: { edit: 'rowIs', list: fix.list, pairs: fix.pairs.map((one) => one.pair) }, because: fix.pairs.map((one) => one.because) };
    case 'describesNothing':
      return { edit: { edit: 'describesNothing', list: fix.list, rows: fix.rows.map((one) => one.row) }, because: fix.rows.map(() => null) };
  }
};

const FixOffer = ({ fix, edit, because }: { readonly fix: Fix; readonly edit: Fixing; readonly because: readonly (Because | null)[] }): ReactElement => {
  const { clicks, editable, proposal, words: screen } = useScreen();
  const words = useFixText();
  const { t, number } = words;
  const [open, setOpen] = useState(false);
  const [off, setOff] = useState<ReadonlySet<number>>(new Set());
  const kind = KINDS[edit.edit];
  const asked = questionOf(edit);
  const count = countOf(edit);
  const picked = keeping(edit, (at) => !off.has(at));
  const held = heldBy(proposal, asked)?.edit;
  const { items, rows } = fix.counts;
  const lines = [
    ...(items[0] === items[1] ? [] : [t('fix.items', { before: number(items[0]), after: number(items[1]) })]),
    ...(rows === null || rows[0].n === rows[1].n ? [] : [t('fix.rows', { before: number(rows[0].n), after: number(rows[1].n), of: number(rows[1].of) })]),
  ];
  return (
    <div className="structure-offer">
      <span>
        {counted(kind.line, count, words)}
        {kind.note === null ? null : ` ${counted(kind.note, count, words)}`}
      </span>
      <Notes lines={lines} />
      <button
        type="button"
        aria-expanded={open}
        onClick={() => {
          setOpen((was) => !was);
        }}
      >
        {counted(['fix.show', 'fix.showOne'], count, words)}
      </button>
      <button
        type="button"
        disabled={!editable || picked === null}
        onClick={() => {
          if (picked !== null) {
            clicks.answer(asked, 'offer', joined(held !== undefined && isFixing(held) ? held : null, picked));
          }
        }}
      >
        {counted(kind.take, picked === null ? 0 : countOf(picked), words)}
      </button>
      {open ? (
        <ul className="structure-fix-instances">
          {instanceWords(edit, t).map((text, at) => {
            const why = because[at] ?? null;
            return (
              <li key={text}>
                <label>
                  <input
                    type="checkbox"
                    checked={!off.has(at)}
                    disabled={!editable}
                    onChange={() => {
                      setOff((was) => {
                        const next = new Set(was);
                        if (!next.delete(at)) {
                          next.add(at);
                        }
                        return next;
                      });
                    }}
                  />{' '}
                  {why === null ? text : `${text} · ${t(BECAUSE[why])}`}
                </label>
              </li>
            );
          })}
        </ul>
      ) : null}
      {clicks.failed === asked ? <p role="alert">{screen.t('structure.click.failed')}</p> : null}
    </div>
  );
};

/** The offers of one folder under its Items question (`items`) or its List question (`list`). */
export const FixOffers = ({ node, on }: { readonly node: SeriesNode; readonly on: 'items' | 'list' }): ReactElement => {
  const words = useFixText();
  const fixes = (node.fixes ?? []).filter((fix) => (fix.fix === 'oneItem') === (on === 'items'));
  return (
    <>
      {fixes.map((fix) => {
        const { edit, because } = offerOf(fix, node.scope.scope);
        return (
          // Keyed by its instances, so a new offer starts with every instance ticked.
          <Row key={canonical(edit)} label={words.t(on === 'items' ? 'fix.q.items' : 'fix.q.rows')}>
            <FixOffer fix={fix} edit={edit} because={because} />
          </Row>
        );
      })}
    </>
  );
};

/** What a correction answers in the changes strip: how many instances it holds. Its question is the screen's, Items or List. */
export const fixAnswer = (edit: Fixing, words: Words): string => {
  const count = { count: words.number(countOf(edit)) };
  switch (edit.edit) {
    case 'oneItem':
      return words.t('fix.joined', count);
    case 'rowIs':
      return words.t('fix.paired', count);
    case 'describesNothing':
      return words.t('fix.left', count);
  }
};

/** How many instances of a correction no longer hold, when any does not. */
export const staleLine = (result: EditResult, words: Words): string | null => {
  const stale = result.stale?.length ?? 0;
  return stale === 0 ? null : counted(['fix.stale', 'fix.staleOne'], stale, words);
};

/** Under a correction in the changes strip: each instance with its tick, and why one no longer holds. Unticking it takes it out. */
export const FixInstances = ({ result, edit }: { readonly result: EditResult; readonly edit: Fixing }): ReactElement => {
  const { clicks, editable } = useScreen();
  const words = useFixText();
  const stale = new Map((result.stale ?? []).map((one) => [one.at, one.why]));
  return (
    <ul className="structure-fix-instances">
      {instanceWords(edit, words.t).map((text, at) => {
        const why = stale.get(at);
        return (
          <li key={text}>
            <label>
              <input
                type="checkbox"
                checked
                disabled={!editable}
                onChange={() => {
                  clicks.answer(questionOf(edit), 'untick', keeping(edit, (other) => other !== at));
                }}
              />{' '}
              {why === undefined ? text : `${text} · ${words.t(STALE[why])}`}
            </label>
          </li>
        );
      })}
    </ul>
  );
};
