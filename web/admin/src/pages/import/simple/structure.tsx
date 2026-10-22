/**
 * Structure: how the archive becomes items and sets. The server's proposal
 * is a suggestion the person corrects. The tree shows each folder that
 * becomes items, each folder left out and each list no folder uses; the
 * inspector shows the selected node's questions, each with the suggested
 * answer (✓) and its rival, and with the counts each answer makes before
 * any click. Above the tree, the status line says whether the plan uses the
 * suggestion, and the count line sums what it makes.
 *
 * This screen only reads: every answer is shown and none can be changed yet.
 * The proposal's parser loads with this screen's chunk and no other.
 */
import { useMemo } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { parseProposal } from '../../../api/import-proposal';
import type { Choice, OutsideFolder, Pick, Proposal, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { ListFile, SetFrom } from '../../../api/import-plan';
import type { ListCandidate, RawProposal, SourceState } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { whereWords } from '../words';
import { MasterDetail, Segmented } from './master-detail';
import type { MasterRow, Segment } from './master-detail';
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
import type { StructureTranslator, TreeNode } from './structure-lines';
import './simple.css';
import './structure.css';

/** A split shows this many of its sets, then how many more. */
const SETS_SHOWN = 3;
/** The endings line shows this many endings. */
const PARTS_SHOWN = 8;

type Words = StructureTranslator;

/** The suggested answer, its rival, and the answer the screen adds ("No list", "No set") when neither is it. */
type Answer = 'chosen' | 'rival' | 'none';

/** A question's control. Nothing can be clicked yet, so every segment is disabled and the suggested one is pressed. */
const Question = ({ label, segments, words }: { readonly label: string; readonly segments: readonly Segment<Answer>[]; readonly words: Words }): ReactElement => (
  <Segmented label={label} segments={segments} value="chosen" disabled suggestedWord={words.t('structure.suggested')} onChange={() => undefined} />
);

/** The segments of a choice whose value may be "none": the chosen, the rival, and "none" when neither is it. */
const segmentsOf = <T, C>(
  choice: Choice<T | null, C>,
  label: (pick: Pick<T, C>) => Segment<Answer>['label'],
  note: (pick: Pick<T, C>) => string | undefined,
  none: string,
): readonly Segment<Answer>[] => {
  const segment = (pick: Pick<T | null, C>, value: Answer, suggested: boolean): Segment<Answer> => {
    const kept = pick.value;
    if (kept === null) {
      return { value, label: none, suggested };
    }
    const text = note({ value: kept, counts: pick.counts });
    return { value, label: label({ value: kept, counts: pick.counts }), suggested, ...(text === undefined ? {} : { note: text }) };
  };
  const shown = [segment(choice.chosen, 'chosen', true), ...(choice.rival === null ? [] : [segment(choice.rival, 'rival', false)])];
  const hasNone = choice.chosen.value === null || choice.rival?.value === null;
  return hasNone ? shown : [...shown, { value: 'none', label: none, suggested: false }];
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

const Offer = ({ text, action }: { readonly text: ReactNode; readonly action: string }): ReactElement => (
  <div className="structure-offer">
    <span>{text}</span>
    <button type="button" disabled>
      {action}
    </button>
  </div>
);

const ItemsQuestion = ({ node, words }: { readonly node: SeriesNode; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  const { chosen, rival } = node.items;
  const counts = chosen.counts;
  const lines = [
    ...(counts.perItem.some((bucket) => bucket.files !== '1' && bucket.count > 0) ? [t('structure.perItem', { buckets: bucketsWords(counts.perItem, t, number) })] : []),
    ...(counts.parts.length === 0
      ? []
      : [t('structure.parts', { parts: counts.parts.slice(0, PARTS_SHOWN).map(([part, items]) => `${part} ${number(items)}`).join(' · ') })]),
    ...(chosen.value.cut === 'places' && counts.namesEndingHere > 0 ? [t('structure.namesEnd', { count: number(counts.namesEndingHere) })] : []),
    ...(counts.otherShapes > 0 ? [t('structure.otherShapes', { count: number(counts.otherShapes) })] : []),
  ];
  return (
    <Row label={t('structure.q.items')}>
      {rival === null ? (
        <span>
          {cutWords(chosen.value, t)} · {itemsWords(counts.items, t, number)}
        </span>
      ) : (
        <Question
          label={t('structure.q.items')}
          words={words}
          segments={[
            { value: 'chosen', label: cutWords(chosen.value, t), suggested: true, note: itemsWords(counts.items, t, number) },
            { value: 'rival', label: cutWords(rival.value, t), suggested: false, note: itemsWords(rival.counts.items, t, number) },
          ]}
        />
      )}
      <Notes lines={lines} />
    </Row>
  );
};

/** The list questions: which list, each row's meaning, its columns and the pairing offers. A series no list names says so. */
const ListQuestions = ({ node, words }: { readonly node: SeriesNode; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  const { list, grain } = node;
  if (list.chosen.value === null && list.rival === null) {
    return <Row label={t('structure.q.list')}>{t('structure.list.nothing')}</Row>;
  }
  const chosen = list.chosen.counts;
  const named = chosen === null ? 0 : rowsNamingAFile(chosen, node);
  const exact = chosen?.best?.paired ?? 0;
  const rowsNote = (candidate: ListCandidate | null, n: number): string | undefined =>
    candidate === null ? undefined : t('structure.list.rows', { n: number(n), of: number(candidate.rows) });
  const listSegments = segmentsOf<ListFile, ListCandidate | null>(
    list,
    (pick) => listName(pick.value),
    (pick) => rowsNote(pick.counts, pick.counts === chosen ? named : (pick.counts?.best?.paired ?? 0)),
    t('structure.list.no'),
  );
  return (
    <>
      <Row label={t('structure.q.list')}>
        <Question label={t('structure.q.list')} words={words} segments={listSegments} />
        <Notes lines={chosen !== null && exact < named ? [t('structure.list.folds', { count: number(exact) })] : []} />
      </Row>
      {chosen === null
        ? null
        : node.pairing.map((offer) => (
            <Row key={JSON.stringify(offer.value)} label={t('structure.q.pair')}>
              <Offer
                text={`${pairWords(offer.value, t)} · ${t('structure.pair.rows', { n: number(offer.counts.rows.n), of: number(offer.counts.rows.of) })}`}
                action={t('structure.pair.use')}
              />
            </Row>
          ))}
      {grain === null || chosen === null || named === 0 ? null : (
        <Row label={t('structure.q.grain')}>
          <Question
            label={t('structure.q.grain')}
            words={words}
            segments={[
              { value: 'chosen', label: grainWords(grain.chosen.value, t), suggested: true, note: itemsWords(grain.chosen.counts.items, t, number) },
              ...(grain.rival === null
                ? []
                : [{ value: 'rival' as const, label: grainWords(grain.rival.value, t), suggested: false, note: itemsWords(grain.rival.counts.items, t, number) }]),
            ]}
          />
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

const SplitOffer = ({ split, words }: { readonly split: Pick<SetFrom, SetCounts>; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  const { sets } = split.counts;
  const shown = sets.slice(0, SETS_SHOWN).map(([name, items]) => t('structure.split.set', { name, items: itemsWords(items, t, number) }));
  const more = sets.length > SETS_SHOWN ? [t('structure.split.more', { count: number(sets.length - SETS_SHOWN) })] : [];
  return (
    <Offer
      text={`${t('structure.split.by', { from: setFromWords(split.value, t) })} ${[...shown, ...more].join(' · ')}`}
      action={t('structure.split.make', { count: number(sets.length) })}
    />
  );
};

const SeriesInspector = ({ node, name, words }: { readonly node: SeriesNode; readonly name: string; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  const { scope } = node;
  const setSegments = segmentsOf<SetFrom, SetCounts>(node.set, (pick) => setName(pick.counts, t, number), (pick) => setNote(pick.value, pick.counts, t, number), t('structure.set.none'));
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <dl className="guided-kv">
        <Row label={t('structure.q.import')}>
          <Question
            label={t('structure.q.import')}
            words={words}
            segments={[
              {
                value: 'chosen',
                label: t('structure.import.yes'),
                suggested: true,
                note: t('structure.import.files', { files: number(scope.files), kinds: kindsWords(scope.kinds, t, number) }),
              },
              { value: 'rival', label: t('structure.import.no'), suggested: false, note: t('structure.import.noCount', { items: itemsWords(seriesItems(node), t, number) }) },
            ]}
          />
        </Row>
        <ItemsQuestion node={node} words={words} />
        <ListQuestions node={node} words={words} />
        <Row label={t('structure.q.set')}>
          <Question label={t('structure.q.set')} words={words} segments={setSegments} />
        </Row>
        {node.splits.map((split) => (
          <Row key={JSON.stringify(split.value)} label={t('structure.q.split')}>
            <SplitOffer split={split} words={words} />
          </Row>
        ))}
      </dl>
    </div>
  );
};

const ArchiveInspector = ({ proposal, where, files, words }: { readonly proposal: Proposal; readonly where: string; readonly files: number; readonly words: Words }): ReactElement => {
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

const EveryInspector = ({ offers, words }: { readonly offers: readonly Pick<SetFrom, SetCounts>[]; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  return (
    <div className="guided-inspect">
      <h4>{t('structure.every.title')}</h4>
      <p>{t('structure.every.note')}</p>
      <dl className="guided-kv">
        {offers.map((offer) => (
          <Row key={JSON.stringify(offer.value)} label={setName(offer.counts, t, number)}>
            <Offer text={setNote(offer.value, offer.counts, t, number)} action={t('structure.every.add')} />
          </Row>
        ))}
      </dl>
    </div>
  );
};

const OutsideInspector = ({ folder, name, words }: { readonly folder: OutsideFolder; readonly name: string; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <p>{folder.reason === 'noMedia' ? t('structure.out.why') : t('structure.out.whyEdit')}</p>
      <p>{t('structure.import.files', { files: number(folder.files), kinds: kindsWords(folder.kinds, t, number) })}</p>
      <button type="button" disabled>
        {t('structure.out.keep')}
      </button>
    </div>
  );
};

const UnusedInspector = ({ list, words }: { readonly list: ListCandidate; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  const paired = list.best?.paired ?? 0;
  const why =
    list.read !== 'ok'
      ? t('structure.unused.error', { error: list.read.error })
      : paired === 0
        ? t('structure.unused.noRow')
        : t('structure.unused.rows', { n: number(paired), of: number(list.rows) });
  return (
    <div className="guided-inspect">
      <h4 className="guided-file">{list.file}</h4>
      <p>{why}</p>
      <button type="button" disabled>
        {t('structure.unused.use')}
      </button>
    </div>
  );
};

/** The tree row of one node: its name and its one line of facts. */
const rowOf = (node: TreeNode, where: string, files: number, proposal: Proposal, words: Words): MasterRow => {
  const { t, number } = words;
  switch (node.kind) {
    case 'archive': {
      const lists = listsIn(proposal);
      return { key: node.key, name: where, facts: t('structure.root.line', { files: number(files), lists: number(lists.used + lists.unused) }) };
    }
    case 'every':
      return {
        key: node.key,
        child: true,
        name: t('structure.every'),
        facts: t('structure.every.line', { sets: node.offers.map((offer) => setName(offer.counts, t, number)).join(', ') }),
      };
    case 'set':
      return {
        key: node.key,
        child: true,
        name: setName(node.node.counts, t, number),
        facts: itemsWords(node.node.counts.sets.reduce((sum, [, count]) => sum + count, 0), t, number),
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
        facts: [itemsWords(seriesItems(node.node), t, number), ...(scope.kinds.length === 0 ? [] : [kindsWords(scope.kinds, t, number)]), listLine].join(' · '),
      };
    }
    case 'outside':
      return {
        key: node.key,
        group: t('structure.group.out'),
        name: folderName(node.folder.scope.folder, where),
        facts: t(node.folder.reason === 'noMedia' ? 'structure.out.noMedia' : 'structure.out.edit', { files: number(node.folder.files) }),
      };
    case 'unused':
      return {
        key: node.key,
        group: t('structure.group.unused'),
        name: <span className="guided-file">{node.list.file}</span>,
        facts:
          node.list.read === 'ok'
            ? t('structure.unused.rows', { n: number(node.list.best?.paired ?? 0), of: number(node.list.rows) })
            : t('structure.unused.unread'),
      };
  }
};

const Inspector = ({ node, where, files, proposal, words }: { readonly node: TreeNode; readonly where: string; readonly files: number; readonly proposal: Proposal; readonly words: Words }): ReactElement => {
  switch (node.kind) {
    case 'archive':
      return <ArchiveInspector proposal={proposal} where={where} files={files} words={words} />;
    case 'every':
      return <EveryInspector offers={node.offers} words={words} />;
    case 'set':
      return (
        <div className="guided-inspect">
          <h4>{setName(node.node.counts, words.t, words.number)}</h4>
          <p>{setNote(node.node.from, node.node.counts, words.t, words.number)}</p>
        </div>
      );
    case 'series':
      return <SeriesInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} words={words} />;
    case 'outside':
      return <OutsideInspector folder={node.folder} name={folderName(node.folder.scope.folder, where)} words={words} />;
    case 'unused':
      return <UnusedInspector list={node.list} words={words} />;
  }
};

const StatusPanel = ({ proposal, words }: { readonly proposal: Proposal; readonly words: Words }): ReactElement => {
  const { t, number } = words;
  const draft = useDraft();
  const status = statusOf(draft.plan, proposal);
  const totals = totalsOf(proposal);
  return (
    <section className="structure-status" aria-live="polite">
      <div className="structure-status-line">
        {status === 'suggested' ? (
          <>
            <p>{t('structure.status.suggested')}</p>
            <button type="button" className="import-primary" disabled>
              {t('structure.status.use')}
            </button>
          </>
        ) : null}
        {status === 'inUse' ? <p>{t('structure.status.inUse', { count: number(totals.changes) })}</p> : null}
        {status === 'changed' ? (
          <>
            <p>{t('structure.status.changed')}</p>
            <span className="structure-actions">
              <button type="button" disabled>
                {t('structure.status.useNew')}
              </button>
              <button type="button" disabled>
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

export const Structure = (): ReactElement => {
  const words = useStructureText();
  const draft = useDraft();
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
  const where = whereWords(draft.plan.sources, words.t, read.data.sources);
  const files = filesIn(read.data.sources);
  const tree = treeOf(proposal);
  const first = tree.find((node) => node.kind === 'series') ?? tree[0];
  return (
    <div className="guided-dials" aria-busy={read.busy}>
      <StatusPanel proposal={proposal} words={words} />
      <MasterDetail label={words.t('structure.tree')} rows={tree.map((node) => rowOf(node, where, files, proposal, words))} initial={first?.key ?? null}>
        {(key) => {
          const node = tree.find((one) => one.key === key);
          return node === undefined ? null : <Inspector node={node} where={where} files={files} proposal={proposal} words={words} />;
        }}
      </MasterDetail>
    </div>
  );
};

export default Structure;
