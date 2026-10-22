/**
 * Structure: how the archive becomes items and sets. The server's proposal
 * is a suggestion the person corrects. The tree (`structure-tree.tsx`)
 * shows each folder that becomes items, each folder left out and each list
 * no folder uses; this inspector, the question zoom, shows the selected
 * node's questions, each with the suggested answer (✓) and its rival, and
 * with the counts each answer makes before any click.
 *
 * Each answer the person picks is one click (`structure-edits.ts`): it turns
 * on at once, its counts follow, and the changes strip under the tree lists
 * every click with what it changed and a way to take it back.
 *
 * A folder can also be written by hand: "Mark an example" opens the hand
 * edit (`structure-hand.tsx`, a lazy chunk of its own). The folder is then
 * "written by you" until the person goes back to the suggestion, and the
 * clicks on it wait meanwhile.
 */
import { Suspense, lazy, useState } from 'react';
import type { ReactElement } from 'react';
import type { Choice, EditResult, Pick, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { Edit, ListFile, SetFrom } from '../../../api/import-plan';
import type { ListCandidate } from '../../../api/import-sections';
import { QUESTIONS, cutEditOf, heldBy } from './structure-edits';
import {
  bucketsWords,
  cutWords,
  folderName,
  grainWords,
  itemsWords,
  keptWords,
  listName,
  pairWords,
  rowsNamingAFile,
  seriesItems,
  setFromWords,
  setName,
  setNote,
} from './structure-lines';
import { ImportQuestion, Notes, Offer, Question, Row, useScreen } from './structure-parts';
import type { Answer } from './structure-parts';
import { StructureTree } from './structure-tree';
import type { InspectorProps } from './structure-tree';

const OtherInspector = lazy(() => import('./structure-nodes').then((module) => ({ default: module.OtherInspector })));
const HandPanel = lazy(() => import('./structure-hand').then((module) => ({ default: module.HandPanel })));
const OwnInspector = lazy(() => import('./structure-hand').then((module) => ({ default: module.OwnInspector })));

/** A split shows this many of its sets, then how many more. */
const SETS_SHOWN = 3;
/** The endings line shows this many endings. */
const PARTS_SHOWN = 8;

/**
 * The answers of a choice. With no edit of the person's, the chosen answer is
 * the suggestion's. With one, the chosen answer is the person's, and the
 * rival is the suggestion's (the proposer makes the answer an edit replaced
 * its rival), so a click on it takes the edit back. `ask` is the edit that
 * asks for an answer; an answer no edit can ask for is not shown. `none` is
 * the answer the screen adds ("No list", "No set") when neither is it.
 */
const answersOf = <T, C>(
  choice: Choice<T | null, C>,
  held: EditResult | undefined,
  ask: (pick: Pick<T, C> | null) => Edit | undefined,
  shown: { readonly label: (pick: Pick<T, C>) => string; readonly note: (pick: Pick<T, C>) => string | undefined; readonly none?: string },
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
    ...(shown.none === undefined || chosen === null || rival === null || noneEdit === undefined ? [] : [answer(null, 'none', false, false, noneEdit)]),
  ];
};

const ItemsQuestion = ({ node, onMark }: { readonly node: SeriesNode; readonly onMark: () => void }): ReactElement => {
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
  );
  return (
    <Row label={t('structure.q.items')}>
      {rival === null ? (
        <span>
          {cutWords(chosen.value, t)} · {itemsWords(counts.items, t, number)}
        </span>
      ) : (
        <Question label={t('structure.q.items')} asked={asked} answers={answers} kind="items" />
      )}
      <Notes lines={lines} />
      <button type="button" disabled={!editable} onClick={onMark}>
        {t('structure.hand.mark')}
      </button>
    </Row>
  );
};

/** The list questions: which list, each row's meaning, its columns and the pairing offers. A series no list names says so. */
const ListQuestions = ({ node }: { readonly node: SeriesNode }): ReactElement => {
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
      {chosen === null ? null : <Row label={t('structure.q.columns')}>{t('structure.columns', { count: number(chosen.columns) })}</Row>}
    </>
  );
};

/**
 * The Set question. Its rival is a column with one value in every row, so a
 * click on it gives the folder's set that title (a `title` edit). While the
 * person's title stands, the folder's own name is the suggestion's answer.
 */
const SetQuestion = ({ node, name }: { readonly node: SeriesNode; readonly name: string }): ReactElement | null => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const { chosen, rival } = node.set;
  const from = chosen.value ?? rival?.value ?? null;
  if (from === null) {
    return <Row label={t('structure.q.set')}>{t('structure.set.none')}</Row>;
  }
  const asked = QUESTIONS.set(from);
  const held = heldBy(proposal, asked);
  const shown = { label: (pick: Pick<SetFrom, SetCounts>) => setName(pick.counts, t, number), note: (pick: Pick<SetFrom, SetCounts>) => setNote(pick.value, pick.counts, t, number), none: t('structure.set.none') };
  const titled = held?.applied === true && held.edit.edit === 'title' ? held.edit : null;
  const answers: readonly Answer[] =
    titled === null
      ? answersOf(node.set, held, (pick) => {
          const own = chosen.value;
          if (pick === null) {
            return own === null ? undefined : { edit: 'dropSet', from: own };
          }
          const title = pick.counts.sets[0]?.[0];
          return own === null ? { edit: 'addSet', from: pick.value } : title === undefined ? undefined : { edit: 'title', from: own, title };
        }, shown)
      : [
          { value: 'chosen', label: titled.title, note: setNote(titled.from, chosen.counts, t, number), draft: false, pressed: true, edit: null },
          { value: 'rival', label: name, draft: true, pressed: false, edit: null },
          { value: 'none', label: shown.none, draft: false, pressed: false, edit: { edit: 'dropSet', from: titled.from } },
        ];
  return (
    <Row label={t('structure.q.set')}>
      <Question label={t('structure.q.set')} asked={asked} answers={answers} kind="set" />
    </Row>
  );
};

const SplitOffer = ({ split }: { readonly split: Pick<SetFrom, SetCounts> }): ReactElement => {
  const { words } = useScreen();
  const { t, number } = words;
  const { sets } = split.counts;
  const shown = sets.slice(0, SETS_SHOWN).map(([name, items]) => t('structure.split.set', { name, items: itemsWords(items, t, number) }));
  const more = sets.length > SETS_SHOWN ? [t('structure.split.more', { count: number(sets.length - SETS_SHOWN) })] : [];
  return (
    <Offer
      text={`${t('structure.split.by', { from: setFromWords(split.value, t) })} ${[...shown, ...more].join(' · ')}`}
      action={t('structure.split.make', { count: number(sets.length) })}
      asked={QUESTIONS.set(split.value)}
      edit={{ edit: 'addSet', from: split.value }}
    />
  );
};

const SeriesInspector = ({ node, name }: { readonly node: SeriesNode; readonly name: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const [marking, setMarking] = useState(false);
  if (marking) {
    return (
      <div className="guided-inspect">
        <h4>{name}</h4>
        <Suspense fallback={<p className="import-slow">{t('shell.loading')}</p>}>
          <HandPanel
            clicks={clicks}
            editable={editable}
            level={node.level}
            scope={node.scope.scope}
            onDone={() => {
              setMarking(false);
            }}
          />
        </Suspense>
      </div>
    );
  }
  const kept = node.kept ?? [];
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <dl className="guided-kv">
        <ImportQuestion scope={node.scope} items={seriesItems(node)} />
        <ItemsQuestion
          node={node}
          onMark={() => {
            setMarking(true);
          }}
        />
        <ListQuestions node={node} />
        {kept.length === 0 ? null : (
          <Row label={t('structure.kept.title')}>
            <Notes lines={kept.map((one) => keptWords(one, t, number))} />
          </Row>
        )}
        <SetQuestion node={node} name={name} />
        {node.splits.map((split) => (
          <Row key={JSON.stringify(split.value)} label={t('structure.q.split')}>
            <SplitOffer split={split} />
          </Row>
        ))}
      </dl>
    </div>
  );
};

const Inspector = ({ node, where, files, folders }: InspectorProps): ReactElement => {
  const { words } = useScreen();
  switch (node.kind) {
    case 'series':
      return <SeriesInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} />;
    case 'own':
      return (
        <Suspense fallback={<p className="import-slow">{words.t('shell.loading')}</p>}>
          <OwnInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} />
        </Suspense>
      );
    default:
      return (
        <Suspense fallback={<p className="import-slow">{words.t('shell.loading')}</p>}>
          <OtherInspector node={node} where={where} files={files} folders={folders} />
        </Suspense>
      );
  }
};

/** Simple's Structure: the question zoom of every node. */
export const Structure = (): ReactElement => <StructureTree Inspector={Inspector} />;

export default Structure;
