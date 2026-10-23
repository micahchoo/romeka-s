/**
 * The set of one node in the rule zoom (P5 design, section 3, control 1):
 * a folder's own set, and a set level's. The editor is a lazy chunk; the
 * Set section and a set level's node load it when they open.
 *
 * Where the proposal offers the choice, a choice is the click Simple sends
 * (`writes.ts`, `offered`): on a suggested folder every set choice the
 * proposer counted, and a title with a description or a set already in
 * Omeka (`title`, `existing`) for a set that is one set. A title and a set
 * in Omeka name one set, so they work only on a folder's own set and on a
 * set over every item; a set level made from values (a split) shows each
 * value with its count, maps each value's set when it is around one folder
 * (`../value-map.tsx`), and can go. A folder written by
 * hand, and a set level kept around one, are the plan's: their set is
 * written there, and the click keeps the folder written by hand.
 */
import { Suspense, lazy, useState } from 'react';
import type { ReactElement } from 'react';
import type { FolderScope, ImportPlan, LevelId, SetFrom, SetSource, SourceRef } from '../../../api/import-plan';
import type { KeptSetNode, NodeEvidence, OwnNode, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { ItemSet } from '../../../api/types';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadCatalog } from '../lists/catalog';
import { usePreview } from '../lists/use-preview';
import { seriesOf, seriesOfTree, seriesUnder, setLevelIn } from '../series';
import { QUESTIONS, heldBy } from '../simple/structure-edits';
import { keptSetName, setName, setNote, setsWords } from '../simple/structure-lines';
import { InShownTree } from '../simple/shown-draft';
import { Row, useScreen } from '../simple/structure-parts';
import { SetQuestion, SplitOffer } from '../simple/structure-questions';
import { useAdvancedText } from './messages';
import { setSeriesSet, setSetLevel } from './plan-edits';
import { sourceChoices, sourceWords } from './source-words';
import type { SourceChoice } from './source-words';

/** The install's item sets, for "a set already in Omeka". */
export const useInstallSets = (): readonly ItemSet[] => {
  const read = useAsync(loadCatalog, []);
  return read.status === 'ready' ? read.data.itemSets : [];
};

/** A set in Omeka by its own title, read from the install; its id when the install has no such set. */
export const installTitle = (sets: readonly ItemSet[], id: number): string => sets.find((set) => set.id === id)?.title ?? `#${String(id)}`;

/** The groups a pattern names, but the item key and the part: what a set per value can read. */
export const groupsOf = (pattern: string | null): readonly string[] =>
  Array.from((pattern ?? '').matchAll(/\(\?<([^>]+)>/gu), (found) => found[1] ?? '').filter((name) => name !== '' && name !== 'identifier' && name !== 'part');

/** The sources one set per value can read: a column of these lists, a folder level, a group of the pattern. */
export const useEachChoices = (plan: ImportPlan, sheets: readonly string[] | null, groups: readonly string[]): readonly SourceChoice[] => {
  const { t } = useAdvancedText();
  const read = usePreview(['sheets'], {});
  const views = read.status === 'ready' ? read.data.sheets.filter((sheet) => sheets === null || sheets.includes(sheet.id)) : [];
  return sourceChoices(plan, views, groups, t).filter(({ source }) => source.from === 'column' || source.from === 'group' || (source.from === 'folder' && source.level !== 'path'));
};

export type SetKind = 'none' | 'title' | 'existing' | 'each';

const kindOf = (set: SetSource | null): SetKind => (set === null ? 'none' : 'title' in set ? 'title' : 'id' in set ? 'existing' : 'each');

const KIND_WORDS = { none: 'adv.set.kind.none', title: 'adv.set.kind.title', existing: 'adv.set.kind.existing', each: 'adv.set.kind.each' } as const;

/**
 * One set source, written as the person makes it: none, a new set by its
 * title and description, a set already in Omeka, or one set per value of a
 * source. `kinds` are the ones this place takes; Use sends the source.
 */
export const SourceEditor = ({
  label,
  kinds,
  current,
  choices,
  onUse,
}: {
  readonly label: string;
  readonly kinds: readonly SetKind[];
  readonly current: SetSource | null;
  readonly choices: readonly SourceChoice[];
  readonly onUse: (set: SetSource | null) => void;
}): ReactElement => {
  const { t } = useAdvancedText();
  const { editable } = useDraft();
  const sets = useInstallSets();
  const [kind, setKind] = useState<SetKind>(() => (kinds.includes(kindOf(current)) ? kindOf(current) : (kinds[0] ?? 'title')));
  const [title, setTitle] = useState(current !== null && 'title' in current ? current.title : '');
  const [about, setAbout] = useState(current !== null && 'title' in current ? (current.description ?? '') : '');
  const [id, setId] = useState(current !== null && 'id' in current ? String(current.id) : '');
  const [source, setSource] = useState(current !== null && 'each' in current && current.each.sources.length === 1 ? JSON.stringify(current.each.sources[0]) : '');
  const made = ((): SetSource | null | undefined => {
    switch (kind) {
      case 'none':
        return null;
      case 'title':
        return title.trim() === '' ? undefined : { title: title.trim(), description: about.trim() === '' ? null : about.trim() };
      case 'existing':
        return id === '' ? undefined : { id: Number(id) };
      case 'each': {
        const found: SourceRef | undefined = choices.find((choice) => choice.value === source)?.source;
        return found === undefined ? undefined : { each: { sources: [found], combine: 'first', names: 'title' } };
      }
    }
  })();
  return (
    <form
      className="adv-set-editor"
      aria-label={label}
      onSubmit={(event) => {
        event.preventDefault();
        if (made !== undefined) {
          onUse(made);
        }
      }}
    >
      <label>
        {label}{' '}
        <select value={kind} disabled={!editable} onChange={(event) => { setKind(kinds.find((one) => one === event.target.value) ?? kind); }}>
          {kinds.map((one) => (
            <option key={one} value={one}>
              {t(KIND_WORDS[one])}
            </option>
          ))}
        </select>
      </label>
      {kind === 'title' ? (
        <>
          <label>
            {t('adv.set.title')} <input type="text" value={title} disabled={!editable} onChange={(event) => { setTitle(event.target.value); }} />
          </label>
          <label>
            {t('adv.set.description')} <textarea rows={2} value={about} disabled={!editable} onChange={(event) => { setAbout(event.target.value); }} />
          </label>
        </>
      ) : null}
      {kind === 'existing' ? (
        <label>
          {t('adv.set.which')}{' '}
          <select value={id} disabled={!editable} onChange={(event) => { setId(event.target.value); }}>
            <option value="">{t('adv.set.choose')}</option>
            {sets.map((set) => (
              <option key={set.id} value={String(set.id)}>
                {set.title ?? `#${String(set.id)}`}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {kind === 'each' ? (
        <label>
          {t('adv.set.from')}{' '}
          <select value={source} disabled={!editable} onChange={(event) => { setSource(event.target.value); }}>
            <option value="">{t('adv.set.choose')}</option>
            {choices.map((choice) => (
              <option key={choice.value} value={choice.value}>
                {choice.label}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      <button type="submit" disabled={!editable || made === undefined}>
        {t('adv.set.use')}
      </button>
    </form>
  );
};

/** The sets a node makes, each with its items, or that they are not counted. */
const Made = ({ counts }: { readonly counts: SetCounts | null }): ReactElement => {
  const { words } = useScreen();
  return <p className="adv-plain">{counts === null ? words.t('structure.keptSet.uncounted') : setsWords(counts, words.t, words.number)}</p>;
};

/** A title with a description, or a set in Omeka, for the one set `from` makes: the click Simple's Set question sends. */
const NameOffered = ({ from }: { readonly from: SetFrom }): ReactElement => {
  const { t } = useAdvancedText();
  const { clicks, proposal, words } = useScreen();
  const sets = useInstallSets();
  const asked = QUESTIONS.title(from);
  const held = heldBy(proposal, asked);
  const edit = held?.applied === true ? held.edit : null;
  const current: SetSource | null =
    edit?.edit === 'title' ? { title: edit.title, description: edit.description ?? null } : edit?.edit === 'existing' ? { id: edit.id } : null;
  return (
    <>
      {current !== null && 'id' in current ? <p className="adv-plain">{t('adv.set.inOmeka', { title: installTitle(sets, current.id) })}</p> : null}
      <SourceEditor
        key={JSON.stringify(current)}
        label={t('adv.set.name')}
        kinds={['title', 'existing']}
        current={current}
        choices={[]}
        onUse={(set) => {
          if (set !== null && 'title' in set) {
            clicks.answer(asked, 'title', { edit: 'title', from, title: set.title, ...(set.description === null ? {} : { description: set.description }) });
          } else if (set !== null && 'id' in set) {
            clicks.answer(asked, 'existing', { edit: 'existing', from, id: set.id });
          }
        }}
      />
      {clicks.failed === asked ? <p role="alert">{words.t('structure.click.failed')}</p> : null}
    </>
  );
};

/** Whether the proposal's tree makes one set here, so a title or a set in Omeka can name it. */
const isOneSet = (set: SetSource | null | undefined): boolean => set !== null && set !== undefined && !('each' in set);

/** A suggested folder's Set section: every set choice the proposer counted, then the name of its one set. */
export const SuggestedSet = ({ node, name }: { readonly node: SeriesNode; readonly name: string }): ReactElement => {
  const { t } = useAdvancedText();
  const { proposal, words } = useScreen();
  const { set } = node;
  const splits = [...node.splits, ...set.others.flatMap((other) => (other.value === null ? [] : [{ value: other.value, counts: other.counts }]))];
  const made = seriesOfTree(proposal.structure).find((series) => series.id === node.level)?.set;
  const from = set.chosen.value;
  return (
    <>
      <dl className="guided-kv">
        <SetQuestion node={node} name={name} />
        {splits.map((split) => (
          <Row key={JSON.stringify(split.value)} label={words.t('structure.q.split')}>
            <SplitOffer split={split} />
          </Row>
        ))}
      </dl>
      {from === null ? null : isOneSet(made) ? <NameOffered from={from} /> : (
        <>
          <p className="adv-plain">{t('adv.set.values')}</p>
          <Made counts={set.chosen.counts} />
        </>
      )}
    </>
  );
};

const ValueMap = lazy(() => import('../value-map'));

/** What each value's set of the split `level` around the folder `series` becomes, once opened, in the plan as the tree shows it. */
const SplitValues = ({ level, series, label }: { readonly level: LevelId; readonly series: LevelId; readonly label: string }): ReactElement => {
  const [open, setOpen] = useState(false);
  return (
    <details className="adv-map" onToggle={(event) => { setOpen(event.currentTarget.open); }}>
      <summary>{label}</summary>
      {open ? (
        <InShownTree level={series}>
          <Suspense fallback={null}>
            <ValueMap home={{ setLevel: level, series }} label={label} />
          </Suspense>
        </InShownTree>
      ) : null}
    </details>
  );
};

/**
 * A set level the proposal counted: a set over every item, or a split
 * around a folder. A split around one folder maps its values; the proposer
 * carries that map into its next proposal (`WrittenMaps`).
 */
export const CountedLevel = ({ node }: { readonly node: Extract<NodeEvidence, { kind: 'set' }> }): ReactElement => {
  const { t } = useAdvancedText();
  const { clicks, editable, proposal, words } = useScreen();
  const level = setLevelIn(proposal.structure, node.level);
  const one = isOneSet(level?.set);
  const [series, ...more] = seriesUnder(proposal.structure, node.level);
  const asked = QUESTIONS.set(node.from);
  const name = setName(node.counts, words.t, words.number);
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <p>{setNote(node.from, node.counts, words.t, words.number)}</p>
      {one ? <NameOffered from={node.from} /> : (
        <>
          {series !== undefined && more.length === 0 ? null : <p className="adv-plain">{t('adv.set.values')}</p>}
          <Made counts={node.counts} />
          {series !== undefined && more.length === 0 ? <SplitValues level={node.level} series={series.id} label={t('adv.fields.map.label', { field: name })} /> : null}
        </>
      )}
      <p className="import-row-actions">
        <button type="button" disabled={!editable} onClick={() => { clicks.answer(asked, 'drop', { edit: 'dropSet', from: node.from }); }}>
          {t('adv.set.remove')}
        </button>
      </p>
      {clicks.failed === asked ? <p role="alert">{words.t('structure.click.failed')}</p> : null}
    </div>
  );
};

/** A hand write of the folder `scope`, which stays written by hand: the click that keeps its `own` edit, with the write. */
const useWrite = (scope: FolderScope): ((write: (plan: ImportPlan) => ImportPlan) => void) => {
  const { clicks } = useScreen();
  return (write) => {
    clicks.answer(QUESTIONS.own(scope), 'own', { edit: 'own', scope }, write);
  };
};

/** A set level kept around a folder written by hand: its own title and description, a set in Omeka, or no level. */
export const KeptLevel = ({ node, scope }: { readonly node: KeptSetNode; readonly scope: FolderScope }): ReactElement => {
  const { t } = useAdvancedText();
  const { words } = useScreen();
  const { plan } = useDraft();
  const sets = useInstallSets();
  const write = useWrite(scope);
  const level: LevelId = node.level;
  return (
    <div className="guided-inspect">
      <h4>{keptSetName(node.set, words.t)}</h4>
      <p>{words.t('structure.keptSet.note')}</p>
      {'id' in node.set ? <p className="adv-plain">{t('adv.set.inOmeka', { title: installTitle(sets, node.set.id) })}</p> : null}
      <Made counts={node.counts} />
      <SourceEditor
        key={JSON.stringify(node.set)}
        label={t('adv.set.level')}
        kinds={'each' in node.set ? ['each', 'title', 'existing', 'none'] : ['title', 'existing', 'none']}
        current={node.set}
        choices={'each' in node.set ? node.set.each.sources.map((source) => ({ value: JSON.stringify(source), label: sourceWords(source, plan, t), source })) : []}
        onUse={(set) => {
          write((plan) => setSetLevel(plan, level, set));
        }}
      />
    </div>
  );
};

/** A folder written by hand: its set, written in the plan. */
export const WrittenSet = ({ node }: { readonly node: OwnNode }): ReactElement => {
  const { t } = useAdvancedText();
  const { plan } = useDraft();
  const sets = useInstallSets();
  const write = useWrite(node.scope.scope);
  const series = seriesOf(plan, node.level);
  const choices = useEachChoices(plan, series.lists.map((list) => list.id), groupsOf(series.grouping.pattern));
  return (
    <>
      {series.set !== null && 'id' in series.set ? <p className="adv-plain">{t('adv.set.inOmeka', { title: installTitle(sets, series.set.id) })}</p> : null}
      <SourceEditor
        key={JSON.stringify(series.set)}
        label={t('adv.set.folder')}
        kinds={['none', 'title', 'existing', 'each']}
        current={series.set}
        choices={choices}
        onUse={(set) => {
          write((now) => setSeriesSet(now, node.level, set));
        }}
      />
    </>
  );
};
