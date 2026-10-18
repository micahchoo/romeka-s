/**
 * The site settings page (`SiteAdmin/IndexController::indexAction`, the
 * `SiteSettingsForm` fields).
 *
 * Upstream saves each field as its own `site_setting` row through a
 * server-rendered form. The port's site document carries the map as
 * `o:site_settings`, a port addition, and the site write stores it back as
 * rows. The controls come from the generated schema in
 * `src/generated/siteSettings.json`.
 *
 * The theme's own settings are a separate map (`o:theme_settings`); the bundled
 * theme declares none, so this page does not show them.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useState } from 'react';
import { api } from '../../api/client';
import { isRecord } from '../../api/parse';
import { encodeSiteSettings } from '../../api/write';
import type { Site, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { SITE_SETTINGS_REGISTRY } from './siteSettingsRegistry';
import { siteSettingDrafts, siteSettingsWire } from './siteSettingsSchema';
import type { SiteSettingDraft, SiteSettingSpec } from './siteSettingsSchema';

/** The template manager is its own chunk, loaded when the user opens it. */
const TemplateManager = lazy(() =>
  import('./TemplateManager').then((module) => ({ default: module.TemplateManager })),
);

const storedSettings = (site: Site): Readonly<Record<string, unknown>> => {
  const value = site.siteSettings;
  return isRecord(value) ? value : {};
};

const isStringList = (value: unknown): value is readonly string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');

/** A friendlier label than the stored wire value, for the known choices. */
const CHOICE_LABELS: Readonly<Record<string, string>> = {
  en: 'English',
  es: 'Spanish',
  item: 'Item',
  media: 'Media',
  original: 'Original',
  '-1': 'Never',
  '0': 'When signed in',
  '1': 'Always',
  sitewide: 'Sitewide',
  'cross-site': 'Cross-site',
  show: 'Show',
  hide: 'Hide',
  light: 'Light',
  dark: 'Dark',
  none: 'None',
  vocab: 'Vocabulary',
  term: 'Term',
  collapsed: 'Collapsed',
  expanded: 'Expanded',
  site_pages: 'Site pages',
  items: 'Items',
  item_sets: 'Item sets',
};

const choiceLabel = (value: string): string => CHOICE_LABELS[value] ?? value;

const SORT_FIELDS = ['title', 'created', 'id'] as const;
const SORT_ORDERS = ['asc', 'desc'] as const;

const SettingControl = ({
  spec,
  value,
  onChange,
}: {
  readonly spec: SiteSettingSpec;
  readonly value: SiteSettingDraft;
  readonly onChange: (value: SiteSettingDraft) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const id = `site-setting-${spec.key}`;
  if (spec.kind === 'boolean') {
    return (
      <p>
        <label htmlFor={id}>
          <input
            id={id}
            type="checkbox"
            checked={value === true}
            onChange={(event) => {
              onChange(event.target.checked);
            }}
          />
          {t(spec.labelKey)}
        </label>
      </p>
    );
  }
  if (spec.kind === 'number') {
    return (
      <p>
        <label htmlFor={id}>{t(spec.labelKey)}</label>
        <input
          id={id}
          type="number"
          min={1}
          value={typeof value === 'number' ? String(value) : ''}
          onChange={(event) => {
            const raw = event.target.value;
            onChange(raw === '' ? '' : Number(raw));
          }}
        />
      </p>
    );
  }
  if (spec.kind === 'choice') {
    return (
      <p>
        <label htmlFor={id}>{t(spec.labelKey)}</label>
        <select
          id={id}
          value={typeof value === 'string' ? value : ''}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        >
          <option value="">{t('common.none')}</option>
          {spec.choices.map((choice) => (
            <option key={choice} value={choice}>
              {choiceLabel(choice)}
            </option>
          ))}
        </select>
      </p>
    );
  }
  if (spec.kind === 'multiChoice') {
    const selected = isStringList(value) ? value : [];
    return (
      <fieldset>
        <legend>{t(spec.labelKey)}</legend>
        {spec.choices.map((choice) => (
          <label key={choice} htmlFor={`${id}-${choice}`}>
            <input
              id={`${id}-${choice}`}
              type="checkbox"
              checked={selected.includes(choice)}
              onChange={(event) => {
                const next = event.target.checked
                  ? [...selected, choice]
                  : selected.filter((item) => item !== choice);
                onChange(next);
              }}
            />
            {choiceLabel(choice)}
          </label>
        ))}
      </fieldset>
    );
  }
  if (spec.kind === 'sort') {
    const sortBy = isRecord(value) && typeof value.sortBy === 'string' ? value.sortBy : spec.sortBy;
    const sortOrder =
      isRecord(value) && typeof value.sortOrder === 'string' ? value.sortOrder : spec.sortOrder;
    return (
      <fieldset>
        <legend>{t(spec.labelKey)}</legend>
        <label htmlFor={`${id}-by`}>{t('sites.sortBy')}</label>
        <select
          id={`${id}-by`}
          value={sortBy}
          onChange={(event) => {
            onChange({ sortBy: event.target.value, sortOrder });
          }}
        >
          {SORT_FIELDS.map((field) => (
            <option key={field} value={field}>
              {field}
            </option>
          ))}
        </select>
        <label htmlFor={`${id}-order`}>{t('sites.sortOrder')}</label>
        <select
          id={`${id}-order`}
          value={sortOrder}
          onChange={(event) => {
            onChange({ sortBy, sortOrder: event.target.value });
          }}
        >
          {SORT_ORDERS.map((order) => (
            <option key={order} value={order}>
              {order}
            </option>
          ))}
        </select>
      </fieldset>
    );
  }
  return (
    <p>
      <label htmlFor={id}>{t(spec.labelKey)}</label>
      <input
        id={id}
        type="text"
        value={typeof value === 'string' ? value : ''}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
    </p>
  );
};

const SiteSettingsEditor = ({
  siteId,
  site,
}: {
  readonly siteId: SiteId;
  readonly site: Site;
}): ReactElement => {
  const { t } = useTranslator();
  const [drafts, setDrafts] = useState<Readonly<Record<string, SiteSettingDraft>>>(() =>
    siteSettingDrafts(SITE_SETTINGS_REGISTRY, storedSettings(site)),
  );
  const { state: saveState, save } = useSave();

  const set = (key: string, value: SiteSettingDraft): void => {
    setDrafts((current) => ({ ...current, [key]: value }));
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    const values = siteSettingsWire(SITE_SETTINGS_REGISTRY, drafts);
    save(async () => {
      await api.patchSite(siteId, encodeSiteSettings(values));
    });
  };

  return (
    <form onSubmit={submit}>
      {SITE_SETTINGS_REGISTRY.settings.map((spec) => (
        <SettingControl
          key={spec.key}
          spec={spec}
          value={drafts[spec.key] ?? ''}
          onChange={(value) => {
            set(spec.key, value);
          }}
        />
      ))}
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

/** The settings form and the on-demand template manager. The manager is a lazy
 * chunk, so the settings route does not carry it until the user opens it. */
const SiteSettingsLoaded = ({
  siteId,
  site,
}: {
  readonly siteId: SiteId;
  readonly site: Site;
}): ReactElement => {
  const { t } = useTranslator();
  const [managing, setManaging] = useState(false);
  return (
    <>
      <SiteSettingsEditor key={String(siteId)} siteId={siteId} site={site} />
      <p>
        <button
          type="button"
          onClick={() => {
            setManaging(true);
          }}
        >
          {t('template.manage')}
        </button>
      </p>
      <Suspense
        fallback={
          <div className="template-dialog-backdrop">
            <div
              className="template-dialog template-dialog-loading"
              role="status"
              aria-label={t('loading.site')}
            />
          </div>
        }
      >
        {managing ? (
          <TemplateManager
            siteId={siteId}
            siteSettings={site.siteSettings}
            onClose={() => {
              setManaging(false);
            }}
          />
        ) : null}
      </Suspense>
    </>
  );
};

/** Loads the site and its settings, then renders the form. */
export const SiteSettingsPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => api.getSite(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteSettingsLoaded key={String(siteId)} siteId={siteId} site={state.data} />;
};
