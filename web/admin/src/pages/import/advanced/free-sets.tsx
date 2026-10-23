/**
 * The person's own sets, on the archive's node (P5 design, section 3,
 * control 2): every free set, not only the first, with the sets it makes
 * and their items, a way to remove it, and a way to add one. A free set
 * crosses the tree ("Exhibition 2018"). The proposer copies free sets as
 * they are, so every write here is the plan's (`writes.ts`, `carried`).
 *
 * A free set lifted from v2 reads its values as v2 did (`names: "text"`):
 * an id when a set has it, else a title. Its row offers both readings, each
 * with its counts before the click, from the set's distinct values and the
 * install's sets. The plan keeps `text` until the person clicks.
 */
import type { ReactElement } from 'react';
import type { ImportPlan, SetNames, SetSource } from '../../../api/import-plan';
import { importApi } from '../../../api/import-routes';
import type { ValueCount } from '../../../api/import-sections';
import type { ItemSet } from '../../../api/types';
import { ErrorNotice } from '../../../app/errors';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { placeHref } from '../screens';
import { allSeries, setsOf, withOnlyFreeSet } from '../series';
import { setsWords, totalsOf } from '../simple/structure-lines';
import { useScreen } from '../simple/structure-parts';
import { useAdvancedText } from './messages';
import { foldTitle, setFreeSets } from './plan-edits';
import { SourceEditor, groupsOf, installTitle, useEachChoices, useInstallSets } from './set-source';
import { sourceWords } from './source-words';

/** The most values one preview page holds (`preview/query.rs`, `MAX_PER_PAGE`). */
const PER_PAGE = 500;

/** Every distinct value `plan`'s sets get, with its items, as the run writes them: after the value map. */
const loadValues = async (draft: ReturnType<typeof useDraft>, plan: ImportPlan): Promise<readonly ValueCount[]> => {
  const values: ValueCount[] = [];
  for (let page = 1; ; page += 1) {
    const read = await importApi.preview(draft.id, ['values'], { plan, values: { page, perPage: PER_PAGE } });
    const sets = read.values.find((one) => one.slot === 'sets')?.values;
    values.push(...(sets?.items ?? []));
    if (sets === undefined || values.length >= sets.total || sets.items.length === 0) {
      return values;
    }
  }
};

/** What each reading of v2's values makes, counted before the click. */
const readings = (values: readonly ValueCount[], sets: readonly ItemSet[]): { readonly titles: { made: number; found: number }; readonly ids: { found: number; none: number } } => {
  const titles = new Set(sets.flatMap((set) => (set.title === null ? [] : [foldTitle(set.title)])));
  const ids = new Set(sets.map((set) => String(set.id)));
  const byTitle = values.filter((value) => titles.has(foldTitle(value.text))).length;
  const byId = values.filter((value) => ids.has(value.text.trim())).length;
  return { titles: { made: values.length - byTitle, found: byTitle }, ids: { found: byId, none: values.length - byId } };
};

/** One free set: where its sets come from, the sets it makes, and Remove. */
const FreeSet = ({ set, at }: { readonly set: SetSource; readonly at: number }): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const { proposal, words } = useScreen();
  const installed = useInstallSets();
  const each = 'each' in set ? set.each : null;
  const trial = withOnlyFreeSet(draft.plan, at);
  const read = useAsync(() => (each === null ? Promise.resolve([]) : loadValues(draft, trial)), [draft.id, draft.detail.revision, JSON.stringify(trial)]);
  const named = (names: SetNames): void => {
    draft.change((plan) => setFreeSets(plan, (sets) => sets.map((one, index) => (index === at && 'each' in one ? { each: { ...one.each, names } } : one))));
  };
  const source =
    'title' in set
      ? t('adv.free.titled', { title: set.title })
      : 'id' in set
        ? t('adv.free.existing', { title: installTitle(installed, set.id) })
        : t('adv.free.each', { source: set.each.sources.map((one) => sourceWords(one, draft.plan, t)).join(', ') });
  const values = read.status === 'ready' ? read.data : [];
  const count = readings(values, installed);
  return (
    <li className="adv-free-set">
      <p>{source}</p>
      {each === null ? (
        <p className="adv-plain">{t('adv.free.items', { count: number(totalsOf(proposal).items) })}</p>
      ) : read.status === 'error' ? (
        <ErrorNotice error={read.error} />
      ) : read.status === 'loading' ? (
        <p className="import-slow">{words.t('shell.loading')}</p>
      ) : (
        <p className="adv-plain">{setsWords({ sets: values.map((value) => [value.text, value.count] as const), rows: null }, words.t, words.number)}</p>
      )}
      {each?.names === 'text' && read.status === 'ready' ? (
        <div className="adv-readings">
          <p className="adv-plain">{t('adv.free.text')}</p>
          <p>
            <button type="button" disabled={!draft.editable} onClick={() => { named('title'); }}>
              {t('adv.free.asTitles')}
            </button>{' '}
            <span className="adv-plain">{t('adv.free.asTitlesCount', { made: number(count.titles.made), found: number(count.titles.found) })}</span>
          </p>
          <p>
            <button type="button" disabled={!draft.editable} onClick={() => { named('id'); }}>
              {t('adv.free.asIds')}
            </button>{' '}
            <span className="adv-plain">{t('adv.free.asIdsCount', { found: number(count.ids.found), none: number(count.ids.none) })}</span>
          </p>
        </div>
      ) : null}
      <button type="button" disabled={!draft.editable} onClick={() => { draft.change((plan) => setFreeSets(plan, (sets) => sets.filter((_, index) => index !== at))); }}>
        {t('adv.free.remove')}
      </button>
    </li>
  );
};

/** The items whose sets a decision of All items changes. */
const decidedItems = (plan: ImportPlan): number =>
  plan.decisions.reduce((sum, decision) => {
    if (decision.kind === 'set') {
      return sum + (decision.slot === 'sets' ? 1 : 0);
    }
    return sum + (decision.kind === 'many' && decision.change.kind === 'set' && decision.change.slot === 'sets' ? decision.entries.length : 0);
  }, 0);

/** The archive's own sets: every free set, and Add a set. */
export const FreeSets = (): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const sets = setsOf(draft.plan);
  const choices = useEachChoices(draft.plan, null, [...new Set(allSeries(draft.plan).flatMap((series) => groupsOf(series.grouping.pattern)))]);
  const decided = decidedItems(draft.plan);
  return (
    <section className="adv-section" aria-label={t('adv.free.title')}>
      <h4>{t('adv.free.title')}</h4>
      <p className="adv-plain">{t('adv.free.plain')}</p>
      {sets.length === 0 ? (
        <p className="import-note">{t('adv.free.none')}</p>
      ) : (
        <ul className="adv-free-sets">
          {sets.map((set, at) => (
            <FreeSet key={`${String(at)}:${JSON.stringify(set)}`} set={set} at={at} />
          ))}
        </ul>
      )}
      {decided === 0 ? null : (
        <p className="adv-plain">
          {decided === 1 ? t('adv.free.decidedOne') : t('adv.free.decided', { count: number(decided) })}{' '}
          <a href={placeHref(draft.id, { mode: 'advanced', screen: 'changes' })}>{t('adv.free.changes')}</a>
        </p>
      )}
      <SourceEditor
        key={sets.length}
        label={t('adv.free.add')}
        kinds={['title', 'existing', 'each']}
        current={null}
        choices={choices}
        onUse={(set) => {
          if (set !== null) {
            draft.change((plan) => setFreeSets(plan, (now) => [...now, set]));
          }
        }}
      />
    </section>
  );
};
