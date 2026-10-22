/**
 * Structure: how the archive becomes items and sets. The server's proposal
 * is a suggestion the person corrects. The tree shows each folder that
 * becomes items, each folder left out and each list no folder uses; the
 * inspector shows the selected node's questions, each with the suggested
 * answer (✓) and its rival, and with the counts each answer makes before
 * any click. Above the tree, the status line says whether the plan uses the
 * suggestion, and the count line sums what it makes.
 *
 * Each answer the person picks is one click (`structure-edits.ts`): it turns
 * on at once, its counts follow, and the changes strip under the tree lists
 * every click with what it changed and a way to take it back. The proposal's
 * parser is a chunk of its own, which loads beside this screen's and no other.
 */
import { Suspense, lazy, use, useMemo, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import type { Choice, EditResult, Pick, Proposal, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { Edit, ListFile, SetFrom } from '../../../api/import-plan';
import type { ListCandidate, RawProposal, SourceState } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import type { Focus } from '../screens';
import { whereWords } from '../words';
import { MasterDetail } from './master-detail';
import type { MasterRow } from './master-detail';
import { QUESTIONS, cutEditOf, editsOn, heldBy, useClicks } from './structure-edits';
import type { Parse } from './structure-edits';
import {
  bucketsWords,
  cutWords,
  folderName,
  grainWords,
  itemsWords,
  kindsWords,
  listName,
  listsIn,
  pairWords,
  rowsNamingAFile,
  seriesItems,
  setFromWords,
  setName,
  setNote,
  statusOf,
  totalsOf,
  treeOf,
  useStructureText,
} from './structure-lines';
import type { TreeNode } from './structure-lines';
import { Notes, Offer, Question, Row, ScreenContext, useScreen } from './structure-parts';
import type { Answer, Words } from './structure-parts';
import './simple.css';
import './structure.css';

/** The proposal's parser, which only this screen and its chunks use, starts loading with the screen. */
const parser = import('../../../api/import-proposal');

/** The strip shows once there is a change; its chunk starts loading with the screen's, so a first click finds it. */
const changesChunk = import('./structure-changes');
const Changes = lazy(() => changesChunk.then((module) => ({ default: module.Changes })));
const OtherInspector = lazy(() => import('./structure-nodes').then((module) => ({ default: module.OtherInspector })));

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


const ItemsQuestion = ({ node }: { readonly node: SeriesNode }): ReactElement => {
  const { proposal, words } = useScreen();
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
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const { scope } = node;
  const asked = QUESTIONS.import(scope.scope);
  const kept = heldBy(proposal, asked)?.applied === true;
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <dl className="guided-kv">
        <Row label={t('structure.q.import')}>
          <Question
            label={t('structure.q.import')}
            asked={asked}
            answers={[
              {
                value: 'chosen',
                label: t('structure.import.yes'),
                draft: !kept,
                pressed: true,
                note: t('structure.import.files', { files: number(scope.files), kinds: kindsWords(scope.kinds, t, number) }),
                edit: null,
              },
              {
                value: 'rival',
                label: t('structure.import.no'),
                draft: kept,
                pressed: false,
                note: t('structure.import.noCount', { items: itemsWords(seriesItems(node), t, number) }),
                edit: kept ? null : { edit: 'leaveOut', scope: scope.scope },
              },
            ]}
          />
        </Row>
        <ItemsQuestion node={node} />
        <ListQuestions node={node} />
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


/** The tree row of one node: its name, its one line of facts, and a mark when a click changed it. */
const rowOf = (node: TreeNode, where: string, files: number, proposal: Proposal, words: Words): MasterRow => {
  const { t, number } = words;
  const facts = (...parts: readonly string[]): string =>
    [...parts, ...(editsOn(node, proposal).length === 0 ? [] : [t('structure.changed')])].join(' · ');
  switch (node.kind) {
    case 'archive': {
      const lists = listsIn(proposal);
      return { key: node.key, name: where, facts: facts(t('structure.root.line', { files: number(files), lists: number(lists.used + lists.unused) })) };
    }
    case 'every':
      return {
        key: node.key,
        child: true,
        name: t('structure.every'),
        facts: facts(t('structure.every.line', { sets: node.offers.map((offer) => setName(offer.counts, t, number)).join(', ') })),
      };
    case 'set':
      return {
        key: node.key,
        child: true,
        name: setName(node.node.counts, t, number),
        facts: facts(itemsWords(node.node.counts.sets.reduce((sum, [, count]) => sum + count, 0), t, number)),
      };
    case 'series': {
      const { scope, list } = node.node;
      const chosen = list.chosen.counts;
      const listLine =
        list.chosen.value === null || chosen === null
          ? t('structure.list.noneShort')
          : t('structure.list.short', { n: number(rowsNamingAFile(chosen, node.node)), of: number(chosen.rows) });
      return {
        key: node.key,
        child: true,
        name: folderName(scope.scope.folder, where),
        facts: facts(itemsWords(seriesItems(node.node), t, number), ...(scope.kinds.length === 0 ? [] : [kindsWords(scope.kinds, t, number)]), listLine),
      };
    }
    case 'outside':
      return {
        key: node.key,
        group: t('structure.group.out'),
        name: folderName(node.folder.scope.folder, where),
        facts: facts(t(node.folder.reason === 'noMedia' ? 'structure.out.noMedia' : 'structure.out.edit', { files: number(node.folder.files) })),
      };
    case 'unused':
      return {
        key: node.key,
        group: t('structure.group.unused'),
        name: <span className="guided-file">{node.list.file}</span>,
        facts: facts(
          node.list.read === 'ok' ? t('structure.unused.rows', { n: number(node.list.best?.paired ?? 0), of: number(node.list.rows) }) : t('structure.unused.unread'),
        ),
      };
  }
};


const Inspector = ({ node, where, files, folders }: { readonly node: TreeNode; readonly where: string; readonly files: number; readonly folders: readonly SeriesNode[] }): ReactElement => {
  const { words } = useScreen();
  if (node.kind === 'series') {
    return <SeriesInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} />;
  }
  return (
    <Suspense fallback={<p className="import-slow">{words.t('shell.loading')}</p>}>
      <OtherInspector node={node} where={where} files={files} folders={folders} />
    </Suspense>
  );
};

const StatusPanel = (): ReactElement => {
  const { clicks, editable, proposal, words } = useScreen();
  const { t, number } = words;
  const draft = useDraft();
  const [kept, setKept] = useState(false);
  const status = statusOf(draft.plan, proposal);
  const totals = totalsOf(proposal);
  return (
    <section className="structure-status" aria-live="polite">
      <div className="structure-status-line">
        {status === 'suggested' ? (
          <>
            <p>{t('structure.status.suggested')}</p>
            <button type="button" className="import-primary" disabled={!editable} onClick={clicks.use}>
              {t('structure.status.use')}
            </button>
          </>
        ) : null}
        {status === 'inUse' ? <p>{t('structure.status.inUse', { count: number(totals.changes) })}</p> : null}
        {status === 'changed' && !kept ? (
          <>
            <p>{t('structure.status.changed')}</p>
            <span className="structure-actions">
              <button type="button" disabled={!editable} onClick={clicks.use}>
                {t('structure.status.useNew')}
              </button>
              <button
                type="button"
                onClick={() => {
                  setKept(true);
                }}
              >
                {t('structure.status.keep')}
              </button>
            </span>
          </>
        ) : null}
      </div>
      <div className="guided-facts">
        <span>
          {t('structure.count.items')} <b>{number(totals.items)}</b>
        </span>
        <span>
          {t('structure.count.sets')} <b>{number(totals.sets)}</b>
        </span>
        <span>
          {t('structure.count.out')}{' '}
          <b>{totals.notImported === 1 ? t('structure.count.foldersOne') : t('structure.count.folders', { count: number(totals.notImported) })}</b>
        </span>
        <span>
          {t('structure.count.changes')} <b>{number(totals.changes)}</b>
        </span>
        {clicks.pending?.slow === true ? (
          <span role="status" className="import-slow">
            {t('structure.counting')}
          </span>
        ) : null}
      </div>
    </section>
  );
};

const parsed = (raw: RawProposal, parse: Parse): Proposal | Error => {
  try {
    return parse(raw);
  } catch (error: unknown) {
    return toError(error);
  }
};

const filesIn = (sources: readonly SourceState[]): number => sources.reduce((sum, source) => sum + source.files, 0);

/** The screen over a proposal that read: the status, the tree with its inspector, and the changes strip. */
const Shown = ({ read, under, sources, parse }: { readonly read: Proposal; readonly under: unknown; readonly sources: readonly SourceState[]; readonly parse: Parse }): ReactElement => {
  const words = useStructureText();
  const draft = useDraft();
  const clicks = useClicks(read, under, parse);
  const proposal = clicks.shown;
  const where = whereWords(draft.plan.sources, words.t, sources);
  const files = filesIn(sources);
  const tree = treeOf(proposal);
  const folders = proposal.nodes.flatMap((node) => (node.kind === 'series' ? [node] : []));
  const focus = useRef(draft.place.mode === 'simple' ? draft.place.focus : undefined);
  const takesFocus = (kind: Focus): boolean => {
    const asked = focus.current === kind;
    if (asked) {
      focus.current = undefined;
    }
    return asked;
  };
  // An old step's address opens the first folder at its question; a person who comes back finds the first node a click changed.
  const changed = focus.current === undefined ? tree.find((node) => editsOn(node, proposal).length > 0) : undefined;
  const first = changed ?? tree.find((node) => node.kind === 'series') ?? tree[0];
  return (
    <ScreenContext.Provider value={{ clicks, editable: draft.editable, proposal, words, takesFocus }}>
      <StatusPanel />
      <MasterDetail label={words.t('structure.tree')} rows={tree.map((node) => rowOf(node, where, files, proposal, words))} initial={first?.key ?? null}>
        {(key) => {
          const node = tree.find((one) => one.key === key);
          return node === undefined ? null : <Inspector node={node} where={where} files={files} folders={folders} />;
        }}
      </MasterDetail>
      <Suspense fallback={null}>
        <Changes tree={tree} where={where} id={draft.id} />
      </Suspense>
    </ScreenContext.Provider>
  );
};

export const Structure = (): ReactElement => {
  const words = useStructureText();
  const { parseProposal } = use(parser);
  const read = usePreview(['proposal'], {});
  const raw = read.status === 'ready' ? read.data.proposal : null;
  const proposal = useMemo(() => (raw === null ? null : parsed(raw, parseProposal)), [raw, parseProposal]);
  if (read.status === 'loading') {
    return (
      <p role="status" className="import-slow">
        {words.t('shell.loading')}
      </p>
    );
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  if (proposal === null || proposal instanceof Error) {
    return <ErrorNotice error={proposal ?? new Error('no proposal')} />;
  }
  return (
    <div className="guided-dials" aria-busy={read.busy}>
      <Shown read={proposal} under={read.data} sources={read.data.sources} parse={parseProposal} />
    </div>
  );
};

export default Structure;
