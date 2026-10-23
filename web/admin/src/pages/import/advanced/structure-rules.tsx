/**
 * Advanced's Structure: Simple's tree (`simple/structure-tree.tsx`), each
 * node with its rules, the rule zoom (P5 design, section 2). The archive's
 * node holds the sources, the files, the defaults and the person's own sets
 * (`free-sets.tsx`). A folder's node holds five sections, each one fact line
 * until opened: Which files, Items, Lists, Fields and Set. A set level's
 * node holds its set (`set-source.tsx`). Opening a section or a set level
 * loads its editor, a lazy chunk.
 *
 * On a suggested folder the questions are Simple's own (every choice the
 * proposer counted, not only the rival), so a choice is the click Simple
 * sends, and a list the proposal offers is that click too
 * (`writes.ts`, `offered`). Every other rule is written into the plan; an
 * owned one makes the folder "written by you" (`writes.ts`, `written`).
 *
 * The tree is the proposal's. A folder the plan does not hold yet (the
 * archive as found, or files that now make another structure) is edited in
 * the proposal's structure: its first write takes that structure, as a
 * click in Simple does (`simple/shown-draft.tsx`).
 */
import { Suspense, lazy, useId, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { OwnNode, SeriesNode } from '../../../api/import-proposal';
import type { LevelId, ListFile } from '../../../api/import-plan';
import { useDraft } from '../draft-context';
import type { Section } from '../screens';
import { seriesOf } from '../series';
import { QUESTIONS } from '../simple/structure-edits';
import { cutWords, folderName, itemsWords, keptSetName, kindsWords, listName, seriesItems, setName } from '../simple/structure-lines';
import { ImportQuestion, useScreen } from '../simple/structure-parts';
import { ItemsQuestion, ListQuestions } from '../simple/structure-questions';
import { InShownTree } from '../simple/shown-draft';
import { StructureTree } from '../simple/structure-tree';
import type { InspectorProps } from '../simple/structure-tree';
import { useAdvancedText } from './messages';
import { entriesOf } from './plan-edits';
import './advanced.css';

const ArchiveFiles = lazy(() => import('./files').then((module) => ({ default: module.ArchiveFiles })));
const ItemsRules = lazy(() => import('./files').then((module) => ({ default: module.ItemsRules })));
const ListsRules = lazy(() => import('./lists').then((module) => ({ default: module.ListsRules })));
const FieldRules = lazy(() => import('./fields').then((module) => ({ default: module.FieldRules })));
const Defaults = lazy(() => import('./fields').then((module) => ({ default: module.Defaults })));
const SuggestedSet = lazy(() => import('./set-source').then((module) => ({ default: module.SuggestedSet })));
const WrittenSet = lazy(() => import('./set-source').then((module) => ({ default: module.WrittenSet })));
const CountedLevel = lazy(() => import('./set-source').then((module) => ({ default: module.CountedLevel })));
const KeptLevel = lazy(() => import('./set-source').then((module) => ({ default: module.KeptLevel })));
const FreeSets = lazy(() => import('./free-sets').then((module) => ({ default: module.FreeSets })));
const OtherInspector = lazy(() => import('../simple/structure-nodes').then((module) => ({ default: module.OtherInspector })));

const Loading = (): ReactElement => {
  const { words } = useScreen();
  return <p className="import-slow">{words.t('shell.loading')}</p>;
};

/** One section of a node: its name and one fact line, and its editor once opened. */
const RuleSection = ({ label, fact, open, toggle, children }: { readonly label: string; readonly fact: string; readonly open: boolean; readonly toggle: () => void; readonly children: ReactNode }): ReactElement => {
  const id = useId();
  return (
    <section className="rule-section" aria-label={label}>
      <h5 className="rule-head">
        <button type="button" aria-expanded={open} aria-controls={id} onClick={toggle}>
          {label}
        </button>{' '}
        <span className="adv-plain">{fact}</span>
      </h5>
      {open ? (
        <div id={id} className="rule-body">
          <Suspense fallback={<Loading />}>{children}</Suspense>
        </div>
      ) : null}
    </section>
  );
};

/** The rule count of the folder `level`, every record kind. */
const useRulesLine = (level: LevelId): string => {
  const { t, number } = useAdvancedText();
  const { slots } = seriesOf(useDraft().plan, level);
  const count = entriesOf(slots.item).length + entriesOf(slots.media).length + entriesOf(slots.itemSet).length;
  return count === 1 ? t('adv.structure.rulesOne') : t('adv.structure.rules', { count: number(count) });
};

/**
 * A folder's sections, open or shut: the one an old address names opens,
 * also when the address changes in the same page, as a reload would open it.
 */
const useSections = (): ((id: Section, fact: string, body: ReactNode) => ReactElement) => {
  const { t } = useAdvancedText();
  const { place } = useDraft();
  const asked = place.mode === 'advanced' ? place.section : undefined;
  const [open, setOpen] = useState<ReadonlySet<Section>>(() => new Set(asked === undefined ? [] : [asked]));
  const [seen, setSeen] = useState(asked);
  if (asked !== seen) {
    setSeen(asked);
    if (asked !== undefined) {
      setOpen(new Set([asked]));
    }
  }
  const toggle = (id: Section): void => {
    setOpen((now) => {
      const next = new Set(now);
      if (!next.delete(id)) {
        next.add(id);
      }
      return next;
    });
  };
  return (id, fact, body) => (
    <RuleSection label={t(`adv.structure.${id}`)} fact={fact} open={open.has(id)} toggle={() => { toggle(id); }}>
      {body}
    </RuleSection>
  );
};

/** A suggested folder: Simple's questions with every choice, then the rules beside them. */
const SuggestedRules = ({ node, name }: { readonly node: SeriesNode; readonly name: string }): ReactElement => {
  const { clicks, words } = useScreen();
  const section = useSections();
  const rules = useRulesLine(node.level);
  const scope = node.scope.scope;
  const { list, set } = node;
  const offered: readonly ListFile[] = [list.chosen, list.rival, ...list.others].flatMap((pick) => (pick === null || pick.value === null ? [] : [pick.value]));
  return (
    <>
      {section('files', words.t('structure.import.files', { files: words.number(node.scope.files), kinds: kindsWords(node.scope.kinds, words.t, words.number) }), (
        <dl className="guided-kv">
          <ImportQuestion scope={node.scope} items={seriesItems(node)} />
        </dl>
      ))}
      {section('items', `${cutWords(node.items.chosen.value, words.t)} · ${itemsWords(seriesItems(node), words.t, words.number)}`, (
        <>
          <dl className="guided-kv">
            <ItemsQuestion node={node} every />
          </dl>
          <ItemsRules level={node.level} />
        </>
      ))}
      {section('lists', list.chosen.value === null ? words.t('structure.list.no') : listName(list.chosen.value), (
        <>
          <dl className="guided-kv">
            <ListQuestions node={node} every />
          </dl>
          <ListsRules
            level={node.level}
            offered={{
              by: 'listChoice',
              lists: offered,
              send: (file) => {
                clicks.answer(QUESTIONS.list(scope), 'offer', { edit: 'list', scope, list: file });
              },
            }}
          />
        </>
      ))}
      {section('fields', rules, <FieldRules level={node.level} />)}
      {section('set', set.chosen.value === null ? words.t('structure.set.none') : setName(set.chosen.counts, words.t, words.number), <SuggestedSet node={node} name={name} />)}
    </>
  );
};

/** A folder written by hand: the same sections, every rule edited in the plan. */
const WrittenRules = ({ node }: { readonly node: OwnNode }): ReactElement => {
  const { words } = useScreen();
  const { plan } = useDraft();
  const section = useSections();
  const rules = useRulesLine(node.level);
  const series = seriesOf(plan, node.level);
  return (
    <>
      {section('files', words.t('structure.import.files', { files: words.number(node.scope.files), kinds: kindsWords(node.scope.kinds, words.t, words.number) }), (
        <dl className="guided-kv">
          <ImportQuestion scope={node.scope} items={node.items.items} />
        </dl>
      ))}
      {section('items', itemsWords(node.items.items, words.t, words.number), <ItemsRules level={node.level} />)}
      {section('lists', series.lists.length === 0 ? words.t('structure.list.no') : series.lists.map((one) => one.file).join(', '), <ListsRules level={node.level} />)}
      {section('fields', rules, <FieldRules level={node.level} />)}
      {section('set', series.set === null ? words.t('structure.set.none') : keptSetName(series.set, words.t), <WrittenSet node={node} />)}
    </>
  );
};

/** A folder of items: its name, the way back when it is written by hand, and its sections. */
const FolderRules = ({ node, name }: { readonly node: SeriesNode | OwnNode; readonly name: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const own = !('list' in node);
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      {own ? (
        <p className="import-row-actions">
          <span>{words.t('structure.own.mark')}</span>{' '}
          <button
            type="button"
            disabled={!editable}
            onClick={() => {
              clicks.answer(QUESTIONS.own(node.scope.scope), 'back', null);
            }}
          >
            {words.t('structure.own.back')}
          </button>
        </p>
      ) : null}
      <InShownTree level={node.level}>{'list' in node ? <SuggestedRules node={node} name={name} /> : <WrittenRules node={node} />}</InShownTree>
    </div>
  );
};

/** The archive: where the files come from and what they are, and the defaults every record gets. */
const ArchiveRules = ({ where }: { readonly where: string }): ReactElement => {
  const { t } = useAdvancedText();
  return (
    <div className="guided-inspect">
      <h4>{where}</h4>
      <Suspense fallback={<Loading />}>
        <ArchiveFiles />
        <div className="adv-section">
          <h4>{t('adv.fields.defaults.title')}</h4>
          <Defaults />
        </div>
        <FreeSets />
      </Suspense>
    </div>
  );
};

const RuleZoom = ({ node, where, files, folders }: InspectorProps): ReactElement => {
  switch (node.kind) {
    case 'archive':
      return <ArchiveRules where={where} />;
    case 'series':
    case 'own':
      return <FolderRules key={node.key} node={node.node} name={folderName(node.node.scope.scope.folder, where)} />;
    case 'set':
      return (
        <Suspense fallback={<Loading />}>
          <CountedLevel key={node.key} node={node.node} />
        </Suspense>
      );
    case 'kept':
      return (
        <Suspense fallback={<Loading />}>
          <KeptLevel key={node.key} node={node.node} scope={node.scope} />
        </Suspense>
      );
    default:
      return (
        <Suspense fallback={<Loading />}>
          <OtherInspector node={node} where={where} files={files} folders={folders} />
        </Suspense>
      );
  }
};

/** Advanced's Structure: the rule zoom of every node. */
export const AdvancedStructure = (): ReactElement => <StructureTree Inspector={RuleZoom} />;

export default AdvancedStructure;
