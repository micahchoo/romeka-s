/**
 * The theme settings page (`SiteAdmin/IndexController::themeSettingsAction`).
 *
 * Upstream builds the form from the theme's config spec and stores the map in
 * `site_setting` under `theme_settings_<theme>`. The port's site document
 * carries that map as `o:theme_settings`, a port addition, and the site write
 * accepts it. The controls come from the bundled theme's schema in
 * `themeSchema.ts`.
 *
 * The resource page blocks the same controller edits
 * (`themeResourcePagesAction`) are not here: the bundled theme declares no
 * resource page blocks, so the port has nothing to configure. Reported.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { isRecord } from '../../api/parse';
import { encodeSiteThemeSettings } from '../../api/write';
import type { Site, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import {
  THEME_SETTINGS_SCHEMA,
  themeSettingDrafts,
  themeSettingsJson,
  type ThemeSettingDraft,
  type ThemeSettingSpec,
} from './themeSchema';

const storedSettings = (site: Site): Readonly<Record<string, unknown>> | null => {
  const value = site.themeSettings;
  return isRecord(value) ? value : null;
};

const SettingControl = ({
  setting,
  value,
  onChange,
}: {
  readonly setting: ThemeSettingSpec;
  readonly value: ThemeSettingDraft;
  readonly onChange: (value: ThemeSettingDraft) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const id = `theme-setting-${setting.key}`;
  if (setting.kind === 'boolean') {
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
          {t(setting.labelKey)}
        </label>
      </p>
    );
  }
  if (setting.kind === 'choice') {
    return (
      <p>
        <label htmlFor={id}>{t(setting.labelKey)}</label>
        <select
          id={id}
          value={typeof value === 'string' ? value : ''}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        >
          <option value="">{t('common.none')}</option>
          {setting.choices?.map((choice) => (
            <option key={choice} value={choice}>
              {choice}
            </option>
          ))}
        </select>
      </p>
    );
  }
  return (
    <p>
      <label htmlFor={id}>{t(setting.labelKey)}</label>
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

const SiteThemeEditor = ({
  siteId,
  site,
}: {
  readonly siteId: SiteId;
  readonly site: Site;
}): ReactElement => {
  const { t } = useTranslator();
  const [drafts, setDrafts] = useState<Readonly<Record<string, ThemeSettingDraft>>>(() =>
    themeSettingDrafts(storedSettings(site)),
  );
  const { state: saveState, save } = useSave();

  const set = (key: string, value: ThemeSettingDraft): void => {
    setDrafts((current) => ({ ...current, [key]: value }));
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    const body = encodeSiteThemeSettings(themeSettingsJson(drafts));
    save(async () => {
      await api.patchSite(siteId, body);
    });
  };

  return (
    <form onSubmit={submit}>
      <p>{t('sites.currentTheme', { theme: site.theme })}</p>
      {THEME_SETTINGS_SCHEMA.map((setting) => (
        <SettingControl
          key={setting.key}
          setting={setting}
          value={drafts[setting.key] ?? ''}
          onChange={(value) => {
            set(setting.key, value);
          }}
        />
      ))}
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

/** Loads the site and its theme settings, then renders the form. */
export const SiteThemePage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => api.getSite(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteThemeEditor key={String(siteId)} siteId={siteId} site={state.data} />;
};
