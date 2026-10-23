/**
 * The Structure tree, which both zooms draw: the status line (with why
 * there is no suggestion when the plan check refused the proposer's tree),
 * the count line, the tree of nodes and the changes strip under it, over one
 * proposal read and its clicks (`structure-edits.ts`). What the inspector
 * beside the tree shows is the zoom's: Simple's questions
 * (`structure.tsx`) or Advanced's rules (`advanced/structure-rules.tsx`).
 * The proposal's parser is a chunk of its own, which loads beside the
 * screen's and no other.
 */
import { Suspense, lazy, use, useMemo, useState } from 'react';
import type { ComponentType, ReactElement } from 'react';
import type { Proposal, SeriesNode } from '../../../api/import-proposal';
import type { RawProposal, SourceState } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { pickNode, usePickedNode } from '../picked-node';
import type { Focus } from '../screens';
import { whereWords } from '../words';
import { MasterDetail } from './master-detail';
import type { MasterRow } from './master-detail';
import { editsOn, useClicks } from './structure-edits';
import type { Parse } from './structure-edits';
import { columnChangesOf, folderName, itemsWords, keptSetName, kindsWords, listsIn, rowsNamingAFile, seriesItems, setName, statusOf, totalsOf, treeOf, useStructureText } from './structure-lines';
import type { TreeNode } from './structure-lines';
import { ScreenContext, useScreen } from './structure-parts';
import type { Words } from './structure-parts';
import './simple.css';
import './structure.css';

/** The proposal's parser, which only this screen and its chunks use, starts loading with the screen. */
const parser = import('../../../api/import-proposal');

/** The strip shows once there is a change; its chunk starts loading with the screen's, so a first click finds it. */
const changesChunk = import('./structure-changes');
const Changes = lazy(() => changesChunk.then((module) => ({ default: module.Changes })));

/** What a zoom's inspector is given for the selected node. */
export interface InspectorProps {
  readonly node: TreeNode;
  /** The archive's name. */
  readonly where: string;
  /** The files of every source. */
  readonly files: number;
  /** Every suggested series of the proposal. */
  readonly folders: readonly SeriesNode[];
}

/** The tree row of one node: its name, its one line of facts, and a mark when a click changed it. */
const rowOf = (node: TreeNode, where: string, files: number, proposal: Proposal, moved: ReadonlySet<string>, words: Words): MasterRow => {
  const { t, number } = words;
  const changed = editsOn(node, proposal).length > 0 || (node.kind === 'series' && moved.has(node.node.level));
  const facts = (...parts: readonly string[]): string => [...parts, ...(changed ? [t('structure.changed')] : [])].join(' · ');
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
    case 'kept':
      return { key: node.key, child: true, name: keptSetName(node.node.set, t), facts: facts(t('structure.own.line')) };
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
    case 'own': {
      const { scope, items } = node.node;
      return {
        key: node.key,
        child: true,
        name: folderName(scope.scope.folder, where),
        facts: [itemsWords(items.items, t, number), ...(scope.kinds.length === 0 ? [] : [kindsWords(scope.kinds, t, number)]), t('structure.own.line')].join(' · '),
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

const StatusPanel = (): ReactElement => {
  const { clicks, editable, proposal, words } = useScreen();
  const { t, number } = words;
  const draft = useDraft();
  const [kept, setKept] = useState(false);
  const status = statusOf(draft.plan, proposal);
  const counted = totalsOf(proposal);
  const totals = { ...counted, changes: counted.changes + columnChangesOf(draft.plan, proposal).length };
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
      {proposal.refused === undefined ? null : <p className="import-note">{t('structure.status.refused', { reason: proposal.refused.reason })}</p>}
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

interface ShownProps {
  readonly read: Proposal;
  readonly under: unknown;
  readonly sources: readonly SourceState[];
  readonly parse: Parse;
  readonly Inspector: ComponentType<InspectorProps>;
}

/** The screen over a proposal that read: the status, the tree with its inspector, and the changes strip. */
const Shown = ({ read, under, sources, parse, Inspector }: ShownProps): ReactElement => {
  const words = useStructureText();
  const draft = useDraft();
  const clicks = useClicks(read, under, parse);
  const proposal = clicks.shown;
  const where = whereWords(draft.plan.sources, words.t, sources);
  const files = filesIn(sources);
  const tree = treeOf(proposal);
  const moved = new Set(columnChangesOf(draft.plan, proposal).map((change) => change.node.level));
  const folders = proposal.nodes.flatMap((node) => (node.kind === 'series' ? [node] : []));
  const picked = usePickedNode();
  // The question an old step's address asks for, until it takes the focus; a new old address in the same page asks again.
  const asked = draft.place.mode === 'simple' ? draft.place.focus : undefined;
  const [focus, setFocus] = useState(asked);
  const [seen, setSeen] = useState(asked);
  if (asked !== seen) {
    setSeen(asked);
    setFocus(asked);
  }
  const takesFocus = (kind: Focus): boolean => {
    const taken = focus === kind;
    if (taken) {
      setFocus(undefined);
    }
    return taken;
  };
  // The node the address names opens; else an old step's address opens the first folder at its question, and a person who comes back finds the first node a click changed.
  const named = tree.find((node) => node.key === picked);
  const changed = focus === undefined ? tree.find((node) => editsOn(node, proposal).length > 0) : undefined;
  const first = named ?? changed ?? tree.find((node) => node.kind === 'series' || node.kind === 'own') ?? tree[0];
  return (
    <ScreenContext.Provider value={{ clicks, editable: draft.editable, proposal, words, focus, takesFocus }}>
      <StatusPanel />
      <MasterDetail
        label={words.t('structure.tree')}
        rows={tree.map((node) => rowOf(node, where, files, proposal, moved, words))}
        initial={first?.key ?? null}
        onSelect={(key) => {
          pickNode(draft.id, draft.place, key);
        }}
      >
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

/** The Structure screen of one zoom: the proposal read, then the tree with `Inspector` beside it. */
export const StructureTree = ({ Inspector }: { readonly Inspector: ComponentType<InspectorProps> }): ReactElement => {
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
      <Shown read={proposal} under={read.data} sources={read.data.sources} parse={parseProposal} Inspector={Inspector} />
    </div>
  );
};
