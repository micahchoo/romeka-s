/**
 * Advanced, Fields: the defaults every new record gets, then one rule per
 * field for the record kind chosen (items, media or item sets), and a field
 * to give a rule of its own.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { RECORD_KINDS } from '../../../api/import-plan';
import type { NamedSlot, RecordKind } from '../../../api/import-plan';
import { api } from '../../../api/client';
import type { ResourceTemplate, Site, User } from '../../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../../app/all-pages';
import { ErrorNotice } from '../../../app/errors';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { fieldOptions, optionFor, useProperties } from './field-names';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey } from './messages';
import { addRule, entriesOf, setDefaults } from './plan-edits';
import { RuleCard } from './rule-card';
import { Setting } from './setting';
import { sourceChoices } from './source-words';
import { slotsOf } from '../series';
import './advanced.css';

const KIND_WORDS: Readonly<Record<RecordKind, AdvancedMessageKey>> = {
  item: 'adv.fields.kind.item',
  media: 'adv.fields.kind.media',
  itemSet: 'adv.fields.kind.itemSet',
};

/** The named slots each record kind can have a rule for (`docs/import-v2.md`, "The derive"). */
const NAMED_OF: Readonly<Record<RecordKind, readonly NamedSlot[]>> = {
  item: ['sets', 'template', 'class', 'owner', 'visibility', 'sites', 'itemKey'],
  media: ['template', 'class', 'owner', 'visibility', 'mediaOrder', 'ingester', 'mediaSource', 'itemKey'],
  itemSet: ['template', 'class', 'owner', 'visibility', 'itemKey'],
};

interface Install {
  readonly templates: readonly ResourceTemplate[];
  readonly sites: readonly Site[];
  /** Empty when this user may not list users. */
  readonly users: readonly User[];
}

const loadInstall = async (): Promise<Install> => {
  const [templates, sites, users] = await Promise.all([
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listSites({ page, perPage: PER_PAGE })),
    allPages((page) => api.listUsers({ page, perPage: PER_PAGE })).catch(() => []),
  ]);
  return { templates, sites, users };
};

const Defaults = (): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const install = useAsync(loadInstall, []);
  const { defaults } = draft.plan;
  const known = install.status === 'ready' ? install.data : { templates: [], sites: [], users: [] };
  return (
    <>
      {install.status === 'error' ? <ErrorNotice error={install.error} /> : null}
      <Setting label={t('adv.fields.visibility.label')} plain={t('adv.fields.visibility.plain')}>
        {(described) => (
          <select
            aria-label={t('adv.fields.visibility.label')}
            aria-describedby={described}
            value={defaults.visibility}
            disabled={!draft.editable}
            onChange={(event) => {
              const visibility = event.target.value === 'public' ? 'public' : 'private';
              draft.change((plan) => setDefaults(plan, { visibility }));
            }}
          >
            <option value="private">{t('adv.fields.visibility.private')}</option>
            <option value="public">{t('adv.fields.visibility.public')}</option>
          </select>
        )}
      </Setting>
      <Setting label={t('adv.fields.owner.label')} plain={t('adv.fields.owner.plain')}>
        {(described) => (
          <select
            aria-label={t('adv.fields.owner.label')}
            aria-describedby={described}
            value={String(defaults.owner)}
            disabled={!draft.editable}
            onChange={(event) => {
              const owner = event.target.value === 'runner' ? 'runner' : Number(event.target.value);
              draft.change((plan) => setDefaults(plan, { owner }));
            }}
          >
            <option value="runner">{t('adv.fields.owner.runner')}</option>
            {known.users.map((user) => (
              <option key={user.id} value={String(user.id)}>
                {user.name} ({user.email})
              </option>
            ))}
            {typeof defaults.owner === 'number' && !known.users.some((user) => user.id === defaults.owner) ? (
              <option value={String(defaults.owner)}>{t('adv.fields.owner.id', { id: defaults.owner })}</option>
            ) : null}
          </select>
        )}
      </Setting>
      <Setting label={t('adv.fields.template.label')} plain={t('adv.fields.template.plain')}>
        {(described) => (
          <select
            aria-label={t('adv.fields.template.label')}
            aria-describedby={described}
            value={defaults.template === null ? '' : String(defaults.template)}
            disabled={!draft.editable}
            onChange={(event) => {
              const template = event.target.value === '' ? null : Number(event.target.value);
              draft.change((plan) => setDefaults(plan, { template }));
            }}
          >
            <option value="">{t('adv.fields.template.none')}</option>
            {known.templates.map((template) => (
              <option key={template.id} value={String(template.id)}>
                {template.label}
              </option>
            ))}
          </select>
        )}
      </Setting>
      <Setting label={t('adv.fields.sites.label')} plain={t('adv.fields.sites.plain')}>
        {(described) =>
          known.sites.length === 0 ? (
            <p className="adv-plain">{t('adv.fields.sites.none')}</p>
          ) : (
            <ul className="adv-list" aria-describedby={described}>
              {known.sites.map((site) => (
                <li key={site.id}>
                  <label className="adv-check">
                    <input
                      type="checkbox"
                      checked={defaults.sites.includes(site.id)}
                      disabled={!draft.editable}
                      onChange={(event) => {
                        const on = event.target.checked;
                        draft.change((plan) =>
                          setDefaults(plan, { sites: on ? [...plan.defaults.sites, site.id] : plan.defaults.sites.filter((id) => id !== site.id) }),
                        );
                      }}
                    />{' '}
                    {site.title}
                  </label>
                </li>
              ))}
            </ul>
          )
        }
      </Setting>
    </>
  );
};

const Rules = ({ kind }: { readonly kind: RecordKind }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const properties = useProperties();
  const read = usePreview(['sheets'], {});
  const known = properties.status === 'ready' ? properties.data : [];
  const sheets = read.status === 'ready' ? read.data.sheets : [];
  const pattern = read.status === 'ready' ? read.data.pattern : 'missing';
  const groups = typeof pattern === 'object' && 'groups' in pattern ? pattern.groups : [];
  const choices = sourceChoices(draft.plan, sheets, groups, t);
  const rules = entriesOf(slotsOf(draft.plan)[kind]);
  const options = fieldOptions(NAMED_OF[kind], known, t).filter((option) => !rules.some(([slot]) => slot === option.value));
  return (
    <>
      {properties.status === 'error' ? <ErrorNotice error={properties.error} /> : null}
      {read.status === 'error' ? <ErrorNotice error={read.error} /> : null}
      {rules.length === 0 ? <p className="import-note">{t('adv.fields.rules.none')}</p> : null}
      {rules.map(([slot, rule]) => (
        <RuleCard key={slot} kind={kind} slot={slot} rule={rule} properties={known} choices={choices} />
      ))}
      <Setting label={t('adv.fields.rules.add')} plain={t('adv.fields.rules.addPlain')}>
        {(described) => (
          <select
            aria-label={t('adv.fields.rules.add')}
            aria-describedby={described}
            value=""
            disabled={!draft.editable || draft.plan.action.action === 'delete'}
            onChange={(event) => {
              const slot = optionFor(options, event.target.value);
              if (slot !== null) {
                draft.change((plan) => addRule(plan, kind, slot));
              }
            }}
          >
            <option value="">{t('adv.fields.rules.choose')}</option>
            {options.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        )}
      </Setting>
      {draft.plan.action.action === 'delete' ? <p className="adv-plain">{t('adv.fields.rules.delete')}</p> : null}
    </>
  );
};

export const AdvancedFields = (): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const [kind, setKind] = useState<RecordKind>('item');
  return (
    <div className="adv-section">
      <h4>{t('adv.fields.defaults.title')}</h4>
      <Defaults />
      <h4>{t('adv.fields.rules.title')}</h4>
      <p className="adv-plain">{t('adv.fields.rules.plain')}</p>
      <div className="import-mode" role="group" aria-label={t('adv.fields.kind.label')}>
        {RECORD_KINDS.map((one) => (
          <button key={one} type="button" aria-pressed={one === kind} onClick={() => { setKind(one); }}>
            {t(KIND_WORDS[one], { count: number(entriesOf(slotsOf(draft.plan)[one]).length) })}
          </button>
        ))}
      </div>
      <Rules kind={kind} />
    </div>
  );
};

export default AdvancedFields;

