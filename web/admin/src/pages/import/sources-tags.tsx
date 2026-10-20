/**
 * The Sources step's second half: the metadata tags the server found in the
 * matched files, each mapped to a field by the user (nothing is suggested),
 * and which source wins when the spreadsheet and the files disagree.
 */
import type { ReactElement } from 'react';
import type { RowsPreview, TagPreview } from '../../api/import-preview';
import type { ImportPlan, MetaRule } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTermLabels } from '../../i18n/termText';
import { fieldOptions, loadProperties } from './field-options';
import type { FieldOption } from './field-options';
import { useImportText } from './messages';
import type { ImportMessageKey } from './messages';
import { withMetaRule } from './plan-changes';
import type { TabProps } from './tabs';

type Change = TabProps['onChange'];

const ON_TEXT: Readonly<Record<MetaRule['on'], ImportMessageKey>> = {
  item: 'tags.onItem',
  media: 'tags.onMedia',
  both: 'tags.onBoth',
};

const FROM_TEXT: Readonly<Record<MetaRule['from'], ImportMessageKey>> = {
  first: 'tags.fromFirst',
  part: 'tags.fromPart',
  all: 'tags.fromAll',
};

const ONS = ['item', 'media', 'both'] as const;
const FROMS = ['first', 'part', 'all'] as const;

/** Which file an item value comes from; a media value always comes from its own file. */
const FromCell = ({
  tag,
  rule,
  editable,
  onSet,
}: {
  readonly tag: string;
  readonly rule: MetaRule | undefined;
  readonly editable: boolean;
  readonly onSet: (rule: MetaRule) => void;
}): ReactElement => {
  const { t } = useImportText();
  const onItem = rule !== undefined && rule.on !== 'media';
  return (
    <td>
      <select
        aria-label={t('tags.fromFor', { tag })}
        value={rule?.from ?? 'first'}
        disabled={!editable || !onItem}
        onChange={(event) => {
          const from = FROMS.find((entry) => entry === event.target.value);
          if (rule !== undefined && from !== undefined) {
            const base = { tag: rule.tag, term: rule.term, on: rule.on, from };
            onSet(from === 'part' ? { ...base, part: rule.part ?? '' } : base);
          }
        }}
      >
        {FROMS.map((from) => (
          <option key={from} value={from}>
            {t(FROM_TEXT[from])}
          </option>
        ))}
      </select>
      {rule?.from === 'part' && onItem ? (
        <input
          aria-label={t('tags.partFor', { tag })}
          className="import-part"
          value={rule.part ?? ''}
          disabled={!editable}
          onChange={(event) => { onSet({ ...rule, part: event.target.value }); }}
        />
      ) : null}
    </td>
  );
};

const TagRow = ({
  tag,
  matched,
  rule,
  fields,
  editable,
  onChange,
}: {
  readonly tag: TagPreview;
  readonly matched: number;
  readonly rule: MetaRule | undefined;
  readonly fields: readonly FieldOption[];
  readonly editable: boolean;
  readonly onChange: Change;
}): ReactElement => {
  const { t, number } = useImportText();
  const set = (next: MetaRule | null): void => {
    onChange((plan) => withMetaRule(plan, tag.tag, next));
  };
  return (
    <tr>
      <td>
        {tag.tag} <small className="query-count">{t(tag.kind === 'exif' ? 'tags.exif' : 'tags.file')}</small>
      </td>
      <td className="cell-num">{t('tags.foundOf', { found: number(tag.found), matched: number(matched) })}</td>
      <td className="import-mono">{tag.sample}</td>
      <td>
        <select
          aria-label={t('tags.fieldFor', { tag: tag.tag })}
          value={rule?.term ?? ''}
          disabled={!editable}
          onChange={(event) => {
            const term = event.target.value;
            set(term === '' ? null : { ...(rule ?? { tag: tag.tag, on: 'item', from: 'first' }), term });
          }}
        >
          <option value="">{t('tags.notMapped')}</option>
          {fields.map((field) => (
            <option key={field.term} value={field.term}>
              {t('tags.fieldOption', { label: field.label, term: field.term })}
            </option>
          ))}
        </select>
      </td>
      <td>
        <select
          aria-label={t('tags.onFor', { tag: tag.tag })}
          value={rule?.on ?? 'item'}
          disabled={!editable || rule === undefined}
          onChange={(event) => {
            const on = ONS.find((entry) => entry === event.target.value);
            if (rule !== undefined && on !== undefined) {
              set({ ...rule, on });
            }
          }}
        >
          {ONS.map((on) => (
            <option key={on} value={on}>
              {t(ON_TEXT[on])}
            </option>
          ))}
        </select>
      </td>
      <FromCell tag={tag.tag} rule={rule} editable={editable} onSet={set} />
    </tr>
  );
};

const Priority = ({ plan, editable, onChange }: Pick<TabProps, 'plan' | 'editable' | 'onChange'>): ReactElement => {
  const { t } = useImportText();
  const choices = [
    ['csv', 'sources.useSheet'],
    ['files', 'sources.useFiles'],
  ] as const;
  return (
    <fieldset disabled={!editable}>
      <legend>{t('sources.priority')}</legend>
      {choices.map(([priority, key]) => (
        <label key={priority} className="import-radio">
          <input
            type="radio"
            name="import-priority"
            checked={plan.priority === priority}
            onChange={() => { onChange((current) => ({ ...current, priority })); }}
          />{' '}
          {t(key)}
        </label>
      ))}
    </fieldset>
  );
};

export const TagsSection = ({
  plan,
  tags,
  matched,
  columns,
  editable,
  onChange,
}: {
  readonly plan: ImportPlan;
  readonly tags: readonly TagPreview[] | undefined;
  readonly matched: number;
  readonly columns: RowsPreview['columns'];
  readonly editable: boolean;
  readonly onChange: Change;
}): ReactElement => {
  const { t } = useImportText();
  const labels = useTermLabels();
  const properties = useAsync(loadProperties, []);
  const fields = properties.status === 'ready' ? fieldOptions(properties.data, columns, labels) : [];
  return (
    <>
      <h3>{t('sources.tagsHeading')}</h3>
      <p>{t('sources.tagsLede')}</p>
      <Priority plan={plan} editable={editable} onChange={onChange} />
      {properties.status === 'error' ? <ErrorNotice error={properties.error} /> : null}
      {tags === undefined ? (
        <p role="status">{t('archive.loadingPreview')}</p>
      ) : (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>{t('tags.tag')}</th>
                <th className="cell-num">{t('tags.found')}</th>
                <th>{t('tags.sample')}</th>
                <th>{t('tags.field')}</th>
                <th>{t('tags.on')}</th>
                <th>{t('tags.from')}</th>
              </tr>
            </thead>
            <tbody>
              {tags.map((tag) => (
                <TagRow
                  key={tag.tag}
                  tag={tag}
                  matched={matched}
                  rule={plan.meta.find((rule) => rule.tag === tag.tag)}
                  fields={fields}
                  editable={editable}
                  onChange={onChange}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="query-count">{t('tags.dateNote')}</p>
    </>
  );
};
