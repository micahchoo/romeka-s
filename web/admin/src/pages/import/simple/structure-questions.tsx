/**
 * The questions of a folder of items, which both zooms ask: Items, List
 * (with what each row describes, its columns, where each column goes and
 * the pairing offers), Set and the splits. Each answer is a click
 * (`structure-edits.ts`) that sends the edit Simple sends. Simple's question zoom shows the suggestion and its
 * rival; Advanced's rule zoom passes `every` and shows every other choice
 * the proposer counted too.
 */
import { Suspense, lazy } from 'react';
import type { ReactElement } from 'react';
import type { Choice, EditResult, Pick, RollUp, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { Edit, ListFile, SetFrom } from '../../../api/import-plan';
import type { ListCandidate } from '../../../api/import-sections';
import { useDraft } from '../draft-context';
import { withColumnLevel } from './list-plan';
import { QUESTIONS, cutEditOf, heldBy } from './structure-edits';
import { bucketsWords, cutWords, fileRowsOf, filesWords, grainWords, itemsWords, listName, pairWords, rollUpWords, rowsNamingAFile, setFromWords, setName, setNote, setsWords, variesWords } from './structure-lines';
import type { PlacedColumn } from './structure-lines';
import { Notes, Offer, Question, Row, useScreen } from './structure-parts';
import type { Answer } from './structure-parts';

/** The corrections a folder is offered: a chunk of their own, since most folders have none. */
const FixOffers = lazy(() => import('./structure-fixes').then((module) => ({ default: module.FixOffers })));

/** The offers of `node` under one question, once their chunk loads. */
const Fixes = ({ node, on }: { readonly node: SeriesNode; readonly on: 'items' | 'list' }): ReactElement | null =>
  node.fixes === undefined ? null : (
    <Suspense fallback={null}>
      <FixOffers node={node} on={on} />
    </Suspense>
  );

/** The endings line shows this many endings. */
const PARTS_SHOWN = 8;

/**
 * The answers of a choice. With no edit of the person's, the chosen answer is
 * the suggestion's. With one, the chosen answer is the person's, and the
 * rival is the suggestion's (the proposer makes the answer an edit replaced
 * its rival), so a click on it takes the edit back. `ask` is the edit that
 * asks for an answer; an answer no edit can ask for is not shown. `none` is
 * the answer the screen adds ("No list", "No set") when neither is it. With
 * `every`, as the rule zoom asks, every other choice the proposer counted is
 * an answer too.
 */
const answersOf = <T, C>(
  choice: Choice<T | null, C>,
  held: EditResult | undefined,
  ask: (pick: Pick<T, C> | null) => Edit | undefined,
  shown: { readonly label: (pick: Pick<T, C>) => string; readonly note: (pick: Pick<T, C>) => string | undefined; readonly none?: string },
  every = false,
): readonly Answer[] => {
  const mine = held?.applied === true;
  const kept = (pick: Pick<T | null, C>): Pick<T, C> | null => (pick.value === null ? null : { value: pick.value, counts: pick.counts });
  const answer = (pick: Pick<T, C> | null, value: string, draft: boolean, pressed: boolean, edit: Edit | null): Answer => {
    if (pick === null) {
      return { value, label: shown.none ?? '', draft, pressed, edit };
    }
    const note = shown.note(pick);
    return { value, label: shown.label(pick), draft, pressed, edit, ...(note === undefined ? {} : { note }) };
  };
  const chosen = kept(choice.chosen);
  const rival = choice.rival === null ? undefined : kept(choice.rival);
  const rivalEdit = rival === undefined || mine ? null : ask(rival);
  const noneEdit = ask(null);
  return [
    answer(chosen, 'chosen', !mine, true, null),
    ...(rival === undefined || rivalEdit === undefined ? [] : [answer(rival, 'rival', mine, false, rivalEdit)]),
    ...(every
      ? choice.others.flatMap((other, at) => {
          const pick = kept(other);
          const edit = pick === null ? undefined : ask(pick);
          return edit === undefined ? [] : [answer(pick, `other:${String(at)}`, false, false, edit)];
        })
      : []),
    ...(shown.none === undefined || chosen === null || rival === null || noneEdit === undefined ? [] : [answer(null, 'none', false, false, noneEdit)]),
  ];
};

export const ItemsQuestion = ({ node, onMark, every = false }: { readonly node: SeriesNode; readonly onMark?: () => void; readonly every?: boolean }): ReactElement => {
  const { editable, proposal, words } = useScreen();
  const { t, number } = words;
  const { chosen, rival } = node.items;
  const counts = chosen.counts;
  const scope = node.scope.scope;
  const asked = QUESTIONS.items(scope);
  const lines = [
    ...(counts.perItem.some((bucket) => bucket.files !== '1' && bucket.count > 0) ? [t('structure.perItem', { buckets: bucketsWords(counts.perItem, t, number) })] : []),
    ...(counts.parts.length === 0
      ? []
      : [t('structure.parts', { parts: counts.parts.slice(0, PARTS_SHOWN).map(([part, items]) => `${part} ${number(items)}`).join(' · ') })]),
    ...(chosen.value.cut === 'places' && counts.namesEndingHere > 0 ? [t('structure.namesEnd', { count: number(counts.namesEndingHere) })] : []),
    ...(counts.otherShapes > 0 ? [t('structure.otherShapes', { count: number(counts.otherShapes) })] : []),
  ];
  const answers = answersOf(
    node.items,
    heldBy(proposal, asked),
    (pick) => {
      const cut = pick === null ? null : cutEditOf(pick.value);
      return cut === null ? undefined : { edit: 'items', scope, cut };
    },
    { label: (pick) => cutWords(pick.value, t), note: (pick) => itemsWords(pick.counts.items, t, number) },
    every,
  );
  return (
    <>
      <Row label={t('structure.q.items')}>
        {rival === null && (!every || node.items.others.length === 0) ? (
          <span>
            {cutWords(chosen.value, t)} · {itemsWords(counts.items, t, number)}
          </span>
        ) : (
          <Question label={t('structure.q.items')} asked={asked} answers={answers} kind="items" />
        )}
        <Notes lines={lines} />
        {onMark === undefined ? null : (
          <button type="button" disabled={!editable} onClick={onMark}>
            {t('structure.hand.mark')}
          </button>
        )}
      </Row>
      <Fixes node={node} on="items" />
    </>
  );
};

/** The list questions: which list, each row's meaning, its columns and the pairing offers. A series no list names says so. */
export const ListQuestions = ({ node, every = false }: { readonly node: SeriesNode; readonly every?: boolean }): ReactElement => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const { list, grain } = node;
  const scope = node.scope.scope;
  if (list.chosen.value === null && list.rival === null) {
    return <Row label={t('structure.q.list')}>{t('structure.list.nothing')}</Row>;
  }
  const chosen = list.chosen.counts;
  const named = chosen === null ? 0 : rowsNamingAFile(chosen, node);
  const exact = chosen?.best?.paired ?? 0;
  const listAsked = QUESTIONS.list(scope);
  const listAnswers = answersOf<ListFile, ListCandidate | null>(
    list,
    heldBy(proposal, listAsked),
    (pick) => ({ edit: 'list', scope, list: pick?.value ?? null }),
    {
      label: (pick) => listName(pick.value),
      note: (pick) =>
        pick.counts === null ? undefined : t('structure.list.rows', { n: number(pick.counts === chosen ? named : (pick.counts.best?.paired ?? 0)), of: number(pick.counts.rows) }),
      none: t('structure.list.no'),
    },
    every,
  );
  const file = list.chosen.value;
  const grainAnswers =
    grain === null || file === null
      ? []
      : answersOf(grain, heldBy(proposal, QUESTIONS.grain(file)), (pick) => (pick === null ? undefined : { edit: 'grain', list: file, describes: pick.value }), {
          label: (pick) => grainWords(pick.value, t),
          note: (pick) => itemsWords(pick.counts.items, t, number),
        });
  return (
    <>
      <Row label={t('structure.q.list')}>
        <Question label={t('structure.q.list')} asked={listAsked} answers={listAnswers} kind="list" />
        <Notes lines={chosen !== null && exact < named ? [t('structure.list.folds', { count: number(exact) })] : []} />
      </Row>
      {chosen === null || file === null
        ? null
        : node.pairing.map((offer) => (
            <Row key={JSON.stringify(offer.value)} label={t('structure.q.pair')}>
              <Offer
                text={`${pairWords(offer.value, t)} · ${t('structure.pair.rows', { n: number(offer.counts.rows.n), of: number(offer.counts.rows.of) })}`}
                action={t('structure.pair.use')}
                asked={QUESTIONS.pair(file)}
                edit={{ edit: 'pair', list: file, rule: offer.value }}
              />
            </Row>
          ))}
      {chosen === null || file === null ? null : <Fixes node={node} on="list" />}
      {grain === null || chosen === null || file === null || named === 0 ? null : (
        <Row label={t('structure.q.grain')}>
          <Question label={t('structure.q.grain')} asked={QUESTIONS.grain(file)} answers={grainAnswers} />
          <Notes
            lines={[
              ...(grain.chosen.counts.rowsNamingNoFile > 0 ? [t('structure.grain.noFile', { count: number(grain.chosen.counts.rowsNamingNoFile) })] : []),
              ...(grain.chosen.counts.groupsWithManyRows > 0 ? [t('structure.grain.many', { count: number(grain.chosen.counts.groupsWithManyRows) })] : []),
            ]}
          />
        </Row>
      )}
      {chosen === null ? null : <ColumnQuestions node={node} columns={chosen.columns} />}
    </>
  );
};

/** On the item, then on each file: the order the segments keep whichever the proposer picks. */
const LEVELS: readonly RollUp[] = ['item', 'file'];

/**
 * Where one column goes: on the item or on each file, each answer with the
 * items or files it gives a value, the proposer's pick marked, and how the
 * column varies inside items. A click writes the column's rules (no edit
 * records it), through the Structure clicks like every other answer.
 */
export const RollUpQuestion = ({ column }: { readonly column: PlacedColumn }): ReactElement => {
  const { words } = useScreen();
  const { t, number } = words;
  const { rollUp, sheet, list, level } = column;
  const ref = { index: rollUp.index, header: rollUp.header };
  const picks = [rollUp.pick.chosen, ...(rollUp.pick.rival === null ? [] : [rollUp.pick.rival])];
  const answers = LEVELS.flatMap((value): readonly Answer[] => {
    const pick = picks.find((one) => one.value === value);
    return pick === undefined
      ? []
      : [
          {
            value,
            label: rollUpWords(value, t),
            note: value === 'item' ? itemsWords(pick.counts.items, t, number) : filesWords(pick.counts.files, t, number),
            draft: value === rollUp.pick.chosen.value,
            pressed: value === level,
            edit: null,
            write: (plan) => withColumnLevel(plan, sheet, ref, value),
          },
        ];
  });
  return (
    <>
      <Question label={t('structure.q.rollUp', { column: rollUp.header })} asked={QUESTIONS.column(list, rollUp.index)} answers={answers} />
      <Notes lines={[variesWords(rollUp.varies, t, number)]} />
    </>
  );
};

/** The list's columns, and on a list whose rows describe files, where each column a rule reads goes. */
const ColumnQuestions = ({ node, columns }: { readonly node: SeriesNode; readonly columns: number }): ReactElement => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const rows = fileRowsOf(useDraft().plan, proposal, node);
  const joined = rows?.sheet.join?.column.index;
  const differ = rows === null ? 0 : node.columns.filter((one) => one.index !== joined && one.varies.groups > 0).length;
  const line =
    differ === 0
      ? t('structure.columns', { count: number(columns) })
      : t(differ === 1 ? 'structure.columns.differOne' : 'structure.columns.differ', { count: number(columns), differ: number(differ) });
  return (
    <>
      <Row label={t('structure.q.columns')}>{line}</Row>
      {(rows?.placed ?? [])
        .filter((one) => one.fileAllowed)
        .map((one) => (
          <Row key={one.rollUp.index} label={one.rollUp.header}>
            <RollUpQuestion column={one} />
          </Row>
        ))}
    </>
  );
};

/**
 * The Set question. Its rival is a column with one value in every row, so a
 * click on it gives the folder's set that title (a `title` edit). While the
 * person's title, or a set in Omeka, stands, the folder's own name is the
 * suggestion's answer. Whether the set is made and what it is are two
 * questions, as the server asks them, so a title never takes back the
 * `addSet` that made the set.
 */
export const SetQuestion = ({ node, name }: { readonly node: SeriesNode; readonly name: string }): ReactElement | null => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const { chosen, rival } = node.set;
  const from = chosen.value ?? rival?.value ?? null;
  if (from === null) {
    return <Row label={t('structure.q.set')}>{t('structure.set.none')}</Row>;
  }
  const asked = QUESTIONS.set(from);
  const named = QUESTIONS.title(from);
  const held = heldBy(proposal, asked);
  const naming = heldBy(proposal, named);
  const shown = { label: (pick: Pick<SetFrom, SetCounts>) => setName(pick.counts, t, number), note: (pick: Pick<SetFrom, SetCounts>) => setNote(pick.value, pick.counts, t, number), none: t('structure.set.none') };
  const titled = naming?.applied === true && (naming.edit.edit === 'title' || naming.edit.edit === 'existing') ? naming.edit : null;
  const answers: readonly Answer[] =
    titled === null
      ? answersOf(node.set, held, (pick) => {
          const own = chosen.value;
          if (pick === null) {
            return own === null ? undefined : { edit: 'dropSet', from: own };
          }
          const title = pick.counts.sets[0]?.[0];
          return own === null ? { edit: 'addSet', from: pick.value } : title === undefined ? undefined : { edit: 'title', from: own, title };
        }, shown).map((answer) => (answer.edit?.edit === 'title' ? { ...answer, asked: named } : answer))
      : [
          {
            value: 'chosen',
            label: titled.edit === 'title' ? titled.title : t('structure.set.existing', { id: String(titled.id) }),
            note: setNote(titled.from, chosen.counts, t, number),
            draft: false,
            pressed: true,
            edit: null,
            asked: named,
          },
          { value: 'rival', label: name, draft: true, pressed: false, edit: null, asked: named },
          { value: 'none', label: shown.none, draft: false, pressed: false, edit: { edit: 'dropSet', from: titled.from } },
        ];
  return (
    <Row label={t('structure.q.set')}>
      <Question label={t('structure.q.set')} asked={asked} answers={answers} kind="set" />
    </Row>
  );
};

export const SplitOffer = ({ split }: { readonly split: Pick<SetFrom, SetCounts> }): ReactElement => {
  const { words } = useScreen();
  const { t, number } = words;
  return (
    <Offer
      text={`${t('structure.split.by', { from: setFromWords(split.value, t) })} ${setsWords(split.counts, t, number)}`}
      action={t('structure.split.make', { count: number(split.counts.sets.length) })}
      asked={QUESTIONS.set(split.value)}
      edit={{ edit: 'addSet', from: split.value }}
    />
  );
};

