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
 * parser loads with this screen's chunk and no other.
 */
import { createContext, useContext, useMemo, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { parseProposal } from '../../../api/import-proposal';
import type { Choice, EditResult, NotApplied, OutsideFolder, Pick, Proposal, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { CutEdit, Edit, ListFile, SetFrom } from '../../../api/import-plan';
import type { ListCandidate, RawProposal, SourceState } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { whereWords } from '../words';
import { MasterDetail, Segmented } from './master-detail';
import type { MasterRow } from './master-detail';
import { QUESTIONS, cutEditOf, editsOn, effectOf, heldBy, nodeOf, questionOf, useClicks } from './structure-edits';
import type { Clicks, Question as Asked } from './structure-edits';
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
import type { StructureMessageKey, StructureTranslator, TreeNode } from './structure-lines';
import './simple.css';
import './structure.css';

/** A split shows this many of its sets, then how many more. */
const SETS_SHOWN = 3;
/** The endings line shows this many endings. */
const PARTS_SHOWN = 8;

type Words = StructureTranslator;

/** One answer of a question: what it shows, and what a click on it sends. */
interface Answer {
  /** `chosen`, `rival` or `none`: its place among the answers. */
  readonly value: string;
  readonly label: string;
  /** What this answer makes, shown before the click. */
  readonly note?: string;
  /** The suggestion's own answer: it carries the ✓. */
  readonly draft: boolean;
  /** The answer the proposal holds now. */
  readonly pressed: boolean;
  /** The edit a click sends; `null` takes the question's edit back. */
  readonly edit: Edit | null;
}

/** What every question reads: the clicks, whether the plan takes edits, the proposal and the words. */
interface Screen {
  readonly clicks: Clicks;
  readonly editable: boolean;
  /** The proposal on screen: the one a click brought back, or the read. */
  readonly proposal: Proposal;
  readonly words: Words;
}

const ScreenContext = createContext<Screen | null>(null);

const useScreen = (): Screen => {
  const screen = useContext(ScreenContext);
  if (screen === null) {
    throw new Error('a question is drawn outside the Structure screen');
  }
  return screen;
};

/** A question's control: the answer the proposal holds is pressed, and a click on another sends its edit. */
const Question = ({ label, asked, answers }: { readonly label: string; readonly asked: Asked; readonly answers: readonly Answer[] }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const waiting = clicks.pending?.asked === asked ? clicks.pending : null;
  const pressed = waiting?.value ?? answers.find((answer) => answer.pressed)?.value ?? null;
  return (
    <>
      <Segmented
        label={label}
        segments={answers.map((answer) => ({
          value: answer.value,
          label: answer.label,
          suggested: answer.draft,
          ...(answer.note === undefined ? {} : { note: answer.note }),
        }))}
        value={pressed}
        disabled={!editable}
        suggestedWord={words.t('structure.suggested')}
        onChange={(value) => {
          const answer = answers.find((one) => one.value === value);
          if (answer !== undefined) {
            clicks.answer(asked, value, answer.edit);
          }
        }}
      />
      {waiting?.slow === true ? (
        <p role="status" className="import-slow">
          {words.t('structure.counting')}
        </p>
      ) : null}
      {clicks.failed === asked ? <p role="alert">{words.t('structure.click.failed')}</p> : null}
    </>
  );
};

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

const Row = ({ label, children }: { readonly label: string; readonly children: ReactNode }): ReactElement => (
  <>
    <dt>{label}</dt>
    <dd>{children}</dd>
  </>
);

const Notes = ({ lines }: { readonly lines: readonly string[] }): ReactElement | null =>
  lines.length === 0 ? null : (
    <div className="structure-notes">
      {lines.map((line) => (
        <p key={line}>{line}</p>
      ))}
    </div>
  );

/** An offer: one button that sends its edit. */
const Offer = ({ text, action, asked, edit }: { readonly text: ReactNode; readonly action: string; readonly asked: Asked; readonly edit: Edit }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  return (
    <div className="structure-offer">
      <span>{text}</span>
      <button
        type="button"
        disabled={!editable}
        onClick={() => {
          clicks.answer(asked, 'offer', edit);
        }}
      >
        {action}
      </button>
      {clicks.failed === asked ? <p role="alert">{words.t('structure.click.failed')}</p> : null}
    </div>
  );
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
        <Question label={t('structure.q.items')} asked={asked} answers={answers} />
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
        <Question label={t('structure.q.list')} asked={listAsked} answers={listAnswers} />
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
      <Question label={t('structure.q.set')} asked={asked} answers={answers} />
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

const ArchiveInspector = ({ where, files }: { readonly where: string; readonly files: number }): ReactElement => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const lists = listsIn(proposal);
  const series = proposal.nodes.filter((node) => node.kind === 'series').length;
  return (
    <div className="guided-inspect">
      <h4>{where}</h4>
      <dl className="guided-kv">
        <Row label={t('structure.root.files')}>{t('structure.root.filesIn', { files: number(files), where })}</Row>
        <Row label={t('structure.root.folders')}>{t('structure.root.foldersLine', { in: number(series), out: number(proposal.outside.length) })}</Row>
        <Row label={t('structure.root.lists')}>{t('structure.root.listsLine', { used: number(lists.used), unused: number(lists.unused) })}</Row>
      </dl>
    </div>
  );
};

const EveryInspector = ({ offers }: { readonly offers: readonly Pick<SetFrom, SetCounts>[] }): ReactElement => {
  const { words } = useScreen();
  const { t, number } = words;
  return (
    <div className="guided-inspect">
      <h4>{t('structure.every.title')}</h4>
      <p>{t('structure.every.note')}</p>
      <dl className="guided-kv">
        {offers.map((offer) => (
          <Row key={JSON.stringify(offer.value)} label={setName(offer.counts, t, number)}>
            <Offer text={setNote(offer.value, offer.counts, t, number)} action={t('structure.every.add')} asked={QUESTIONS.set(offer.value)} edit={{ edit: 'addSet', from: offer.value }} />
          </Row>
        ))}
      </dl>
    </div>
  );
};

/** A folder not imported. One the person left out comes back by taking the edit back; one with nothing to show takes a `keep`. */
const OutsideInspector = ({ folder, name }: { readonly folder: OutsideFolder; readonly name: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const asked = QUESTIONS.import(folder.scope);
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <p>{folder.reason === 'noMedia' ? t('structure.out.why') : t('structure.out.whyEdit')}</p>
      <p>{t('structure.import.files', { files: number(folder.files), kinds: kindsWords(folder.kinds, t, number) })}</p>
      <button
        type="button"
        disabled={!editable}
        onClick={() => {
          clicks.answer(asked, 'keep', folder.reason === 'edit' ? null : { edit: 'keep', scope: folder.scope });
        }}
      >
        {t('structure.out.keep')}
      </button>
      {clicks.failed === asked ? <p role="alert">{t('structure.click.failed')}</p> : null}
    </div>
  );
};

/** A list no folder uses: it may describe any folder that is imported. */
const UnusedInspector = ({ list, folders, where }: { readonly list: ListCandidate; readonly folders: readonly SeriesNode[]; readonly where: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const [folder, setFolder] = useState(0);
  const paired = list.best?.paired ?? 0;
  const why =
    list.read !== 'ok'
      ? t('structure.unused.error', { error: list.read.error })
      : paired === 0
        ? t('structure.unused.noRow')
        : t('structure.unused.rows', { n: number(paired), of: number(list.rows) });
  const scope = folders[folder]?.scope.scope;
  return (
    <div className="guided-inspect">
      <h4 className="guided-file">{list.file}</h4>
      <p>{why}</p>
      {list.read !== 'ok' || scope === undefined ? null : (
        <div className="structure-offer">
          <label>
            {t('structure.unused.folder')}{' '}
            <select
              value={folder}
              disabled={!editable}
              onChange={(event) => {
                setFolder(Number(event.target.value));
              }}
            >
              {folders.map((one, at) => (
                <option key={one.level} value={at}>
                  {folderName(one.scope.scope.folder, where)}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            disabled={!editable}
            onClick={() => {
              clicks.answer(QUESTIONS.list(scope), 'use', { edit: 'list', scope, list: { source: list.source, file: list.file } });
            }}
          >
            {t('structure.unused.use')}
          </button>
          {clicks.failed === QUESTIONS.list(scope) ? <p role="alert">{t('structure.click.failed')}</p> : null}
        </div>
      )}
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
  switch (node.kind) {
    case 'archive':
      return <ArchiveInspector where={where} files={files} />;
    case 'every':
      return <EveryInspector offers={node.offers} />;
    case 'set':
      return (
        <div className="guided-inspect">
          <h4>{setName(node.node.counts, words.t, words.number)}</h4>
          <p>{setNote(node.node.from, node.node.counts, words.t, words.number)}</p>
        </div>
      );
    case 'series':
      return <SeriesInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} />;
    case 'outside':
      return <OutsideInspector folder={node.folder} name={folderName(node.folder.scope.folder, where)} />;
    case 'unused':
      return <UnusedInspector list={node.list} folders={folders} where={where} />;
  }
};

const cutEditWords = (cut: CutEdit, t: Words['t']): string => {
  switch (cut.cut) {
    case 'whole':
      return t('structure.cut.whole');
    case 'places':
      return t('structure.cut.places', { places: String(cut.places) });
    case 'folder':
      return t('structure.cut.folder', { depth: String(cut.depth) });
  }
};

/** An edit as the question it answers and the answer, in the words of the inspector. */
const editWords = (edit: Edit, t: Words['t']): { readonly question: string; readonly answer: string } => {
  switch (edit.edit) {
    case 'leaveOut':
      return { question: t('structure.q.import'), answer: t('structure.import.no') };
    case 'keep':
      return { question: t('structure.q.import'), answer: t('structure.import.yes') };
    case 'items':
      return { question: t('structure.q.items'), answer: cutEditWords(edit.cut, t) };
    case 'list':
      return { question: t('structure.q.list'), answer: edit.list === null ? t('structure.list.no') : listName(edit.list) };
    case 'grain':
      return { question: t('structure.q.grain'), answer: grainWords(edit.describes, t) };
    case 'pair':
      return { question: t('structure.q.pair'), answer: pairWords(edit.rule, t) };
    case 'addSet':
      return { question: t('structure.q.set'), answer: setFromWords(edit.from, t) };
    case 'dropSet':
      return { question: t('structure.q.set'), answer: t('structure.set.none') };
    case 'title':
      return { question: t('structure.q.set'), answer: edit.title };
  }
};

/** Where an edit acts, by the name of the node it names, else by the folder or list it names. */
const placeOf = (edit: Edit, tree: readonly TreeNode[], where: string, t: Words['t']): string => {
  const node = nodeOf(edit, tree);
  if (node?.kind === 'series') {
    return folderName(node.node.scope.scope.folder, where);
  }
  if (node?.kind === 'every') {
    return t('structure.every');
  }
  switch (edit.edit) {
    case 'leaveOut':
    case 'keep':
    case 'items':
    case 'list':
      return folderName(edit.scope.folder, where);
    case 'grain':
    case 'pair':
      return listName(edit.list);
    case 'addSet':
    case 'dropSet':
    case 'title':
      return edit.from.from === 'column' ? listName(edit.from.list) : folderName(edit.from.scope.folder, where);
  }
};

const REASONS: Readonly<Record<NotApplied, StructureMessageKey>> = {
  gone: 'structure.change.gone',
  notOffered: 'structure.change.notOffered',
  replaced: 'structure.change.replaced',
};

/** The changes strip: each click with what it changed, or why it does not apply now, and a way to take it back. */
const Changes = ({ tree, where, id }: { readonly tree: readonly TreeNode[]; readonly where: string; readonly id: Parameters<typeof effectOf>[0] }): ReactElement | null => {
  const { clicks, editable, proposal, words } = useScreen();
  const { t, number } = words;
  if (proposal.edits.length === 0 && clicks.pending?.saved !== true) {
    return null;
  }
  return (
    <section className="structure-changes" aria-label={t('structure.count.changes')}>
      <h3>{t('structure.changes.title', { count: number(proposal.edits.length) })}</h3>
      {clicks.pending?.saved === true ? <p role="status">{t('structure.change.saved')}</p> : null}
      <ul>
        {proposal.edits.map((result) => {
          const { question, answer } = editWords(result.edit, t);
          const effect = effectOf(id, result.edit);
          const after =
            result.reason !== null
              ? t(REASONS[result.reason])
              : effect === undefined
                ? null
                : t('structure.change.effect', { before: number(effect.before), after: number(effect.after) });
          return (
            <li key={JSON.stringify(result.edit)} className={result.applied ? undefined : 'is-waiting'}>
              <span>{t('structure.change.line', { where: placeOf(result.edit, tree, where, t), question, answer })}</span>
              {after === null ? null : <span className="structure-change-effect">{after}</span>}
              <button
                type="button"
                disabled={!editable}
                onClick={() => {
                  clicks.answer(questionOf(result.edit), 'back', null);
                }}
              >
                {t('structure.change.remove')}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
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

const parsed = (raw: RawProposal): Proposal | Error => {
  try {
    return parseProposal(raw);
  } catch (error: unknown) {
    return toError(error);
  }
};

const filesIn = (sources: readonly SourceState[]): number => sources.reduce((sum, source) => sum + source.files, 0);

/** The screen over a proposal that read: the status, the tree with its inspector, and the changes strip. */
const Shown = ({ read, under, sources }: { readonly read: Proposal; readonly under: unknown; readonly sources: readonly SourceState[] }): ReactElement => {
  const words = useStructureText();
  const draft = useDraft();
  const clicks = useClicks(read, under);
  const proposal = clicks.shown;
  const where = whereWords(draft.plan.sources, words.t, sources);
  const files = filesIn(sources);
  const tree = treeOf(proposal);
  const folders = proposal.nodes.flatMap((node) => (node.kind === 'series' ? [node] : []));
  // A person who comes back finds the first node a click changed, else the first folder.
  const first = tree.find((node) => editsOn(node, proposal).length > 0) ?? tree.find((node) => node.kind === 'series') ?? tree[0];
  return (
    <ScreenContext.Provider value={{ clicks, editable: draft.editable, proposal, words }}>
      <StatusPanel />
      <MasterDetail label={words.t('structure.tree')} rows={tree.map((node) => rowOf(node, where, files, proposal, words))} initial={first?.key ?? null}>
        {(key) => {
          const node = tree.find((one) => one.key === key);
          return node === undefined ? null : <Inspector node={node} where={where} files={files} folders={folders} />;
        }}
      </MasterDetail>
      <Changes tree={tree} where={where} id={draft.id} />
    </ScreenContext.Provider>
  );
};

export const Structure = (): ReactElement => {
  const words = useStructureText();
  const read = usePreview(['proposal'], {});
  const raw = read.status === 'ready' ? read.data.proposal : null;
  const proposal = useMemo(() => (raw === null ? null : parsed(raw)), [raw]);
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
      <Shown read={proposal} under={read.data} sources={read.data.sources} />
    </div>
  );
};

export default Structure;
