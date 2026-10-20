/**
 * Step 2, Item sets: which folder level names a set, and per folder whether
 * it becomes one and under what name. The server reads the folders, counts
 * the files and says which names an existing set already has.
 */
import type { ReactElement } from 'react';
import type { SetPreview } from '../../api/import-preview';
import { SET_LEVELS } from '../../api/imports';
import type { ImportPlan, SetLevel } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { useImportText } from './messages';
import { PreviewError } from './preview-error';
import type { ImportMessageKey } from './messages';
import type { TabProps } from './tabs';
import { usePreview } from './use-preview';

const LEVEL_TEXT: Readonly<Record<SetLevel, ImportMessageKey>> = {
  level1: 'level.level1',
  level2: 'level.level2',
  both: 'level.both',
  path: 'level.path',
  group: 'level.group',
};

const withSet = (plan: ImportPlan, set: SetPreview, change: Partial<{ use: boolean; name: string }>): ImportPlan => ({
  ...plan,
  sets: { ...plan.sets, [set.key]: { use: set.use, name: set.name, ...change } },
});

/** Whether the pattern has a `set` group, which the last level choice reads. */
const hasSetGroup = (plan: ImportPlan): boolean => plan.pattern?.includes('(?<set>') === true;

const SetRow = ({
  set,
  editable,
  onChange,
}: {
  readonly set: SetPreview;
  readonly editable: boolean;
  readonly onChange: TabProps['onChange'];
}): ReactElement => {
  const { t, number } = useImportText();
  return (
    <tr>
      <td>
        <input
          type="checkbox"
          aria-label={t('sets.useLabel', { folder: set.key })}
          checked={set.use}
          disabled={!editable}
          onChange={(event) => {
            const use = event.target.checked;
            onChange((plan) => withSet(plan, set, { use }));
          }}
        />
      </td>
      <td className="import-mono">{set.key}</td>
      <td>
        <input
          aria-label={t('sets.nameLabel', { folder: set.key })}
          value={set.name}
          disabled={!editable || !set.use}
          onChange={(event) => {
            const name = event.target.value;
            onChange((plan) => withSet(plan, set, { name }));
          }}
        />
      </td>
      <td className="cell-num">{t('sets.matchedOf', { matched: number(set.matched), total: number(set.total) })}</td>
      <td>
        {!set.use ? (
          <span className="badge badge-muted">{t('sets.skipped', { count: number(set.total) })}</span>
        ) : set.existing ? (
          <span className="badge">{t('sets.existing')}</span>
        ) : (
          <span className="badge badge-accent">{t('sets.new')}</span>
        )}
      </td>
    </tr>
  );
};

/**
 * The server's rows, with this page's unsaved choices laid over them, so a
 * typed name shows at once and the preview catches up behind it.
 */
const withChoices = (sets: readonly SetPreview[], plan: ImportPlan): readonly SetPreview[] =>
  sets.map((set) => {
    const choice = plan.sets[set.key];
    return choice === undefined ? set : { ...set, use: choice.use, name: choice.name };
  });

export const SetsStep = ({ detail, plan, editable, onChange }: TabProps): ReactElement => {
  const { t } = useImportText();
  const state = usePreview(detail.id, plan.pattern === null ? null : { include: ['sets'], plan });
  const sets = state.preview?.sets;
  return (
    <section className="import-sets">
      <p className="lede">{t('sets.lede')}</p>
      <fieldset disabled={!editable}>
        <legend>{t('sets.from')}</legend>
        {SET_LEVELS.map((level) => (
          <label key={level} className="import-radio">
            <input
              type="radio"
              name="import-set-level"
              value={level}
              checked={plan.setLevel === level}
              disabled={level === 'group' && !hasSetGroup(plan)}
              onChange={() => { onChange((current) => ({ ...current, setLevel: level })); }}
            />{' '}
            {t(LEVEL_TEXT[level])}
          </label>
        ))}
      </fieldset>
      {state.error === null ? null : <ErrorNotice error={state.error} />}
      <PreviewError preview={state.preview} />
      {plan.pattern === null || (sets !== undefined && sets.length === 0) ? (
        <p>{t('sets.none')}</p>
      ) : sets === undefined ? (
        <p role="status">{t('archive.loadingPreview')}</p>
      ) : (
        <div className="table-scroll" aria-busy={state.loading}>
          <table>
            <thead>
              <tr>
                <th>{t('sets.use')}</th>
                <th>{t('sets.folder')}</th>
                <th>{t('sets.name')}</th>
                <th className="cell-num">{t('sets.matched')}</th>
                <th>{t('sets.result')}</th>
              </tr>
            </thead>
            <tbody>
              {withChoices(sets, plan).map((set) => (
                <SetRow key={set.key} set={set} editable={editable} onChange={onChange} />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="query-count">{t('sets.reuse')}</p>
    </section>
  );
};
