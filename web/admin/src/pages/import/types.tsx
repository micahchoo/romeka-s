/**
 * Step 3, Item types: one item type per item set in use, an existing
 * template or a new type from a starter. A new type is stored in the plan as
 * `{ starter, name }`; the import creates it.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { SetPreview } from '../../api/import-preview';
import { isNewType } from '../../api/imports';
import type { ImportPlan, NewType, TypeChoice } from '../../api/imports';
import { getStarters } from '../../api/starters';
import type { Starter, StarterId } from '../../api/starters';
import type { Property, ResourceTemplate } from '../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../app/all-pages';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useLocale } from '../../i18n';
import type { Locale } from '../../i18n';
import { useTermLabels } from '../../i18n/termText';
import type { TermLabels } from '../../i18n/termText';
import { starterLabel, starterName } from '../item-types/starter-text';
import { useImportText } from './messages';
import { PreviewError } from './preview-error';
import type { TabProps } from './tabs';
import { usePreview } from './use-preview';

interface TypeOptions {
  readonly templates: readonly ResourceTemplate[];
  readonly properties: ReadonlyMap<number, Property>;
  /** The server's starters by the id the plan stores. */
  readonly starters: ReadonlyMap<StarterId, Starter>;
}

const loadOptions = async (): Promise<TypeOptions> => {
  const [templates, properties, table] = await Promise.all([
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listProperties({ page, perPage: PER_PAGE })),
    getStarters(api),
  ]);
  return {
    templates,
    properties: new Map(properties.map((property) => [Number(property.id), property])),
    starters: new Map(table.starters.map((starter) => [starter.id, starter])),
  };
};

/** A choice as a select value: `t<id>` a template, `n<name>` a new type, `` none. */
const valueOf = (choice: TypeChoice | undefined): string => {
  if (choice === undefined) {
    return '';
  }
  return isNewType(choice) ? `n${choice.name}` : `t${String(choice)}`;
};

const NEW = '__new';

/** The template an item with no choice gets, by the name the server's catalog gives it. */
const DEFAULT_TEMPLATE = 'Base Resource';

const fieldsOf = (
  chosen: TypeChoice | undefined,
  options: TypeOptions,
  labels: TermLabels,
  locale: Locale,
): string => {
  const choice = chosen ?? options.templates.find((entry) => entry.label === DEFAULT_TEMPLATE)?.id;
  if (choice === undefined) {
    return '';
  }
  if (isNewType(choice)) {
    return (options.starters.get(choice.starter)?.fields ?? [])
      .map((field) => `${starterLabel(field, locale)}${field.required ? ' *' : ''}`)
      .join(', ');
  }
  const template = options.templates.find((entry) => entry.id === choice);
  return (template?.properties ?? [])
    .map((entry) => {
      const property = options.properties.get(Number(entry.property.id));
      const label = entry.alternateLabel ?? (property === undefined ? '' : (labels.label(property.term, property.label) ?? property.term));
      return `${label}${entry.isRequired ? ' *' : ''}`;
    })
    .join(', ');
};

const AskType = ({
  set,
  starters,
  onCreate,
  onCancel,
}: {
  readonly set: SetPreview;
  readonly starters: ReadonlyMap<StarterId, Starter>;
  readonly onCreate: (type: NewType) => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const { t, locale } = useImportText();
  const [starter, setStarter] = useState<StarterId | null>(null);
  const [name, setName] = useState('');
  const chosen = starter === null ? undefined : starters.get(starter);
  return (
    <form
      className="import-panel"
      onSubmit={(event) => {
        event.preventDefault();
        if (starter !== null && name.trim() !== '') {
          onCreate({ starter, name: name.trim() });
        }
      }}
    >
      <h3>{t('types.newHeading', { name: set.name })}</h3>
      <p>{t('types.question')}</p>
      <div className="import-chips" role="group" aria-label={t('types.question')}>
        {[...starters].map(([id, entry]) => (
          <button
            key={id}
            type="button"
            aria-pressed={starter === id}
            onClick={() => {
              setStarter(id);
              setName(starterName(entry, locale));
            }}
          >
            {starterName(entry, locale)}
          </button>
        ))}
      </div>
      {chosen === undefined ? null : (
        <>
          <p>
            <label>
              {t('types.name')}
              <input value={name} onChange={(event) => { setName(event.target.value); }} />
            </label>
          </p>
          <p className="muted">
            {t('types.startsWith', {
              fields: chosen.fields
                .map((field) => `${starterLabel(field, locale)}${field.required ? ' *' : ''}`)
                .join(', '),
            })}
          </p>
        </>
      )}
      <div className="import-chips">
        <button type="submit" className="button-primary" disabled={starter === null || name.trim() === ''}>
          {t('types.create')}
        </button>
        <button type="button" onClick={onCancel}>
          {t('types.cancel')}
        </button>
      </div>
    </form>
  );
};

/** Every new type the plan names, once each, for the other sets' selects. */
const newTypesIn = (plan: ImportPlan): readonly NewType[] => {
  const byName = new Map<string, NewType>();
  for (const choice of Object.values(plan.typeFor)) {
    if (isNewType(choice)) {
      byName.set(choice.name, choice);
    }
  }
  return [...byName.values()];
};

const withType = (plan: ImportPlan, key: string, choice: TypeChoice | null): ImportPlan => {
  const others = Object.entries(plan.typeFor).filter(([entry]) => entry !== key);
  return { ...plan, typeFor: Object.fromEntries(choice === null ? others : [...others, [key, choice]]) };
};

const TypesTable = ({
  sets,
  options,
  plan,
  editable,
  onChange,
}: Pick<TabProps, 'plan' | 'editable' | 'onChange'> & { readonly sets: readonly SetPreview[]; readonly options: TypeOptions }): ReactElement => {
  const { t, number } = useImportText();
  const locale = useLocale();
  const labels = useTermLabels();
  const [asking, setAsking] = useState<SetPreview | null>(null);
  const created = newTypesIn(plan);
  const choose = (set: SetPreview, value: string): void => {
    if (value === NEW) {
      setAsking(set);
      return;
    }
    const template = options.templates.find((entry) => `t${String(entry.id)}` === value);
    const fresh = created.find((entry) => `n${entry.name}` === value);
    onChange((current) => withType(current, set.key, template?.id ?? fresh ?? null));
  };
  return (
    <>
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              <th>{t('types.set')}</th>
              <th className="cell-num">{t('types.files')}</th>
              <th>{t('types.type')}</th>
              <th>{t('types.fields')}</th>
            </tr>
          </thead>
          <tbody>
            {sets.map((set) => {
              const choice = plan.typeFor[set.key];
              return (
                <tr key={set.key}>
                  <td>{set.name}</td>
                  <td className="cell-num">{number(set.matched)}</td>
                  <td>
                    <select
                      aria-label={t('types.typeFor', { name: set.name })}
                      value={valueOf(choice)}
                      disabled={!editable}
                      onChange={(event) => { choose(set, event.target.value); }}
                    >
                      <option value="">{t('types.default')}</option>
                      {options.templates.map((template) => (
                        <option key={template.id} value={`t${String(template.id)}`}>
                          {template.label}
                        </option>
                      ))}
                      {created.map((entry) => (
                        <option key={entry.name} value={`n${entry.name}`}>
                          {t('types.newName', { name: entry.name })}
                        </option>
                      ))}
                      <option value={NEW}>{t('types.newChoice')}</option>
                    </select>
                  </td>
                  <td className="import-fields">{fieldsOf(choice, options, labels, locale)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {asking === null ? null : (
        <AskType
          key={asking.key}
          set={asking}
          starters={options.starters}
          onCancel={() => { setAsking(null); }}
          onCreate={(type) => {
            onChange((current) => withType(current, asking.key, type));
            setAsking(null);
          }}
        />
      )}
    </>
  );
};

export const TypesStep = ({ detail, plan, editable, onChange }: TabProps): ReactElement => {
  const { t } = useImportText();
  const options = useAsync(loadOptions, []);
  const preview = usePreview(detail.id, plan.pattern === null ? null : { include: ['sets'], plan });
  const sets = (preview.preview?.sets ?? []).filter((set) => plan.sets[set.key]?.use ?? set.use);
  return (
    <section className="import-types">
      <p className="lede">{t('types.lede')}</p>
      {options.status === 'error' ? <ErrorNotice error={options.error} /> : null}
      {preview.error === null ? null : <ErrorNotice error={preview.error} />}
      <PreviewError preview={preview.preview} />
      {options.status === 'loading' || (preview.preview === null && plan.pattern !== null) ? (
        <p role="status">{t('types.loading')}</p>
      ) : options.status === 'error' ? null : sets.length === 0 ? (
        <p>{t('types.noSets')}</p>
      ) : (
        <TypesTable sets={sets} options={options.data} plan={plan} editable={editable} onChange={onChange} />
      )}
    </section>
  );
};
