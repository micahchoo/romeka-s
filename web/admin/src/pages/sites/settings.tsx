/**
 * The site settings page (`SiteAdmin/IndexController::editAction`): the
 * site's own members (`SiteForm`) and the `SiteSettingsForm` fields, saved
 * together by one partial update as upstream's edit form does.
 *
 * Upstream saves each field as its own `site_setting` row through a
 * server-rendered form. The port's site document carries the map as
 * `o:site_settings`, a port addition, and the site write stores it back as
 * rows. The controls come from the generated schema in
 * `src/generated/siteSettings.json`, one fieldset per group. A stored key the
 * schema leaves out (no renderer reads it) is not in the write, so the partial
 * update keeps its row.
 *
 * The theme's own settings are a separate map (`o:theme_settings`); the bundled
 * theme declares none, so this page does not show them.
 */
import type { ReactElement, ReactNode } from 'react';
import { Suspense, lazy, useState } from 'react';
import { api } from '../../api/client';
import { isRecord } from '../../api/parse';
import { encodeSiteInfo, encodeSiteSettings } from '../../api/write';
import type { Property, Site, SiteId, User } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { SettingControl } from './SettingControl';
import { useSiteSaved } from './SiteFrame';
import { SiteInfoFields, siteInfoOf } from './siteInfo';
import type { SiteInfo } from './siteInfo';
import { SITE_SETTINGS_REGISTRY } from './siteSettingsRegistry';
import { SITE_SETTING_GROUPS, siteSettingDrafts, siteSettingsWire } from './siteSettingsSchema';
import type { SiteSettingDraft, SiteSettingGroup } from './siteSettingsSchema';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof sites>;

/** The template manager is its own chunk, loaded when the user opens it. */
const TemplateManager = lazy(() =>
  import('./TemplateManager').then((module) => ({ default: module.TemplateManager })),
);

const storedSettings = (site: Site): Readonly<Record<string, unknown>> => {
  const value = site.siteSettings;
  return isRecord(value) ? value : {};
};

/** Each group's legend and one-line intro. */
const GROUP_TEXT: Readonly<
  Record<SiteSettingGroup, { readonly legend: MessageId; readonly intro: MessageId }>
> = {
  browsing: { legend: 'siteSettingGroup.browsing', intro: 'siteSettingGroup.browsingIntro' },
  search: { legend: 'siteSettingGroup.search', intro: 'siteSettingGroup.searchIntro' },
  itemPages: { legend: 'siteSettingGroup.itemPages', intro: 'siteSettingGroup.itemPagesIntro' },
  language: { legend: 'siteSettingGroup.language', intro: 'siteSettingGroup.languageIntro' },
  navigation: { legend: 'siteSettingGroup.navigation', intro: 'siteSettingGroup.navigationIntro' },
  maps: { legend: 'siteSettingGroup.maps', intro: 'siteSettingGroup.mapsIntro' },
  advanced: { legend: 'siteSettingGroup.advanced', intro: 'siteSettingGroup.advancedIntro' },
};

/** The group's settings as one fieldset. Advanced starts collapsed, because
 * only an Omeka expert needs it. */
const SettingGroup = ({
  group,
  children,
}: {
  readonly group: SiteSettingGroup;
  readonly children: ReactNode;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const text = GROUP_TEXT[group];
  const intro = <p className="settings-group-intro">{t(text.intro)}</p>;
  if (group === 'advanced') {
    return (
      <details className="settings-group">
        <summary>{t(text.legend)}</summary>
        {intro}
        {children}
      </details>
    );
  }
  return (
    <fieldset className="settings-group">
      <legend>{t(text.legend)}</legend>
      {intro}
      {children}
    </fieldset>
  );
};

interface Loaded {
  readonly site: Site;
  readonly users: readonly User[];
  readonly properties: readonly Property[];
}

const SiteSettingsEditor = ({
  siteId,
  loaded,
}: {
  readonly siteId: SiteId;
  readonly loaded: Loaded;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const siteSaved = useSiteSaved();
  const [info, setInfo] = useState<SiteInfo>(() => siteInfoOf(loaded.site));
  const [drafts, setDrafts] = useState<Readonly<Record<string, SiteSettingDraft>>>(() =>
    siteSettingDrafts(SITE_SETTINGS_REGISTRY, storedSettings(loaded.site)),
  );
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const set = (key: string, value: SiteSettingDraft): void => {
    setDrafts((current) => ({ ...current, [key]: value }));
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    if (info.title.trim() === '') {
      setFormError(t('sites.errorNoTitle'));
      return;
    }
    setFormError(null);
    const values = siteSettingsWire(SITE_SETTINGS_REGISTRY, drafts);
    save(async () => {
      siteSaved(
        await api.patchSite(siteId, { ...encodeSiteInfo(info), ...encodeSiteSettings(values) }),
      );
    });
  };

  return (
    <form onSubmit={submit}>
      <SiteInfoFields info={info} users={loaded.users} onChange={setInfo} />
      {SITE_SETTING_GROUPS.map((group) => (
        <SettingGroup key={group} group={group}>
          {SITE_SETTINGS_REGISTRY.settings
            .filter((spec) => spec.group === group)
            .map((spec) => (
              <SettingControl
                key={spec.key}
                spec={spec}
                value={drafts[spec.key] ?? ''}
                properties={loaded.properties}
                onChange={(value) => {
                  set(spec.key, value);
                }}
              />
            ))}
        </SettingGroup>
      ))}
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

/**
 * The settings form, the on-demand template manager and the delete control.
 * The manager is a lazy chunk, so the settings route does not carry it until
 * the user opens it.
 */
const SiteSettingsLoaded = ({
  siteId,
  loaded,
}: {
  readonly siteId: SiteId;
  readonly loaded: Loaded;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [managing, setManaging] = useState(false);
  return (
    <>
      <SiteSettingsEditor key={String(siteId)} siteId={siteId} loaded={loaded} />
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
            siteSettings={loaded.site.siteSettings}
            onClose={() => {
              setManaging(false);
            }}
          />
        ) : null}
      </Suspense>
      <DeleteControl
        label={t('sites.deleteLabel')}
        rowName={loaded.site.title}
        onConfirm={async () => {
          await api.deleteSite(siteId);
          window.location.hash = '#/sites';
        }}
      />
    </>
  );
};

const load = async (siteId: SiteId): Promise<Loaded> => {
  const [site, users, properties] = await Promise.all([
    api.getSite(siteId),
    api.listUsers({ perPage: 200, sortBy: 'name' }),
    api.listProperties({ perPage: 200, sortBy: 'label' }),
  ]);
  return { site, users: users.items, properties: properties.items };
};

/** Loads the site and the user list the owner select offers, then renders the form. */
export const SiteSettingsPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(() => load(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteSettingsLoaded key={String(siteId)} siteId={siteId} loaded={state.data} />;
};
