import type { ReactElement, ReactNode } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { getSettings, getTimeZones, updateSettings } from '../../api/settings';
import type { GlobalSettings } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus } from '../../app/fields';
import { reloadInstallation } from '../../app/installation';
import { LocaleSelect, localeValue } from '../../app/locale-select';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/**
 * One allow list as the form holds it: `SettingForm`'s comma-separated text,
 * or `null` once its restore button asked for upstream's default.
 */
type ListDraft = string | null;

const listText = (names: readonly string[]): string => names.join(',');

/** `SettingForm`'s filter: split on commas, trim, drop the empty entries. */
const listNames = (draft: ListDraft): readonly string[] | null =>
  draft === null
    ? null
    : draft
        .split(',')
        .map((name) => name.trim())
        .filter((name) => name !== '');

interface Loaded {
  readonly stored: GlobalSettings;
  readonly timeZones: readonly string[];
}

const load = async (): Promise<Loaded> => {
  const [stored, timeZones] = await Promise.all([getSettings(api), getTimeZones(api)]);
  return { stored, timeZones };
};

/**
 * `Admin/SettingController::browseAction`: every `SettingForm` field the port
 * has, each stored through `PATCH /api/settings`. The browser checks the
 * required fields, the email shape and the page size before it sends; the
 * server checks them again.
 */
export const SettingsPage = (): ReactElement => {
  const { t } = useTranslator(common, admin);
  const loaded = useAsync(load, []);
  if (loaded.status === 'loading') {
    return <p role="status">{t('loading.page')}</p>;
  }
  // A caller other than a global admin is refused, as upstream's ACL refuses
  // them the settings controller.
  if (loaded.status === 'error') {
    return <ErrorNotice error={loaded.error} />;
  }
  return <SettingsForm stored={loaded.data.stored} timeZones={loaded.data.timeZones} />;
};

/** A field's one line of help, which its control names as its description. */
const Info = ({ id, children }: { readonly id: string; readonly children: ReactNode }): ReactElement => (
  <>
    <br />
    <small id={id}>{children}</small>
  </>
);

const SettingsForm = ({
  stored,
  timeZones,
}: {
  readonly stored: GlobalSettings;
  readonly timeZones: readonly string[];
}): ReactElement => {
  const { t } = useTranslator(common, admin);
  const [saved, setSaved] = useState(false);
  const [indexJob, setIndexJob] = useState<number | null>(null);
  const [title, setTitle] = useState(stored.installationTitle ?? '');
  const [email, setEmail] = useState(stored.administratorEmail ?? '');
  const [timeZone, setTimeZone] = useState(stored.timeZone);
  const [perPage, setPerPage] = useState(String(stored.perPage));
  const [privateSets, setPrivateSets] = useState(stored.defaultPrivateItemSets);
  const [versionNotices, setVersionNotices] = useState(stored.versionNotifications);
  const [reindex, setReindex] = useState(false);
  const [locale, setLocale] = useState(localeValue(stored.locale));
  const [altTextProperty, setAltTextProperty] = useState(stored.mediaAltTextProperty ?? '');
  const [disabled, setDisabled] = useState(stored.disableFileValidation);
  const [allowPrivate, setAllowPrivate] = useState(stored.importAllowPrivateUrls);
  const [notesPublic, setNotesPublic] = useState(stored.annotationsPublic);
  const [mediaTypes, setMediaTypes] = useState<ListDraft>(listText(stored.mediaTypeWhitelist));
  const [extensions, setExtensions] = useState<ListDraft>(listText(stored.extensionWhitelist));
  const { state: saveState, save } = useSave();

  const edited =
    <T,>(set: (value: T) => void) =>
    (value: T): void => {
      setSaved(false);
      set(value);
    };

  return (
    <section>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          setSaved(false);
          setIndexJob(null);
          save(async () => {
            const answer = await updateSettings(api, {
              installationTitle: title,
              administratorEmail: email,
              timeZone,
              perPage: Number(perPage),
              defaultPrivateItemSets: privateSets,
              versionNotifications: versionNotices,
              indexFulltextSearch: reindex,
              locale,
              mediaAltTextProperty: altTextProperty,
              mediaTypeWhitelist: listNames(mediaTypes),
              extensionWhitelist: listNames(extensions),
              disableFileValidation: disabled,
              importAllowPrivateUrls: allowPrivate,
              annotationsPublic: notesPublic,
            });
            setTitle(answer.installationTitle ?? '');
            setEmail(answer.administratorEmail ?? '');
            setTimeZone(answer.timeZone);
            setPerPage(String(answer.perPage));
            setPrivateSets(answer.defaultPrivateItemSets);
            setVersionNotices(answer.versionNotifications);
            setReindex(false);
            setIndexJob(answer.indexJob);
            setMediaTypes(listText(answer.mediaTypeWhitelist));
            setExtensions(listText(answer.extensionWhitelist));
            setAltTextProperty(answer.mediaAltTextProperty ?? '');
            setDisabled(answer.disableFileValidation);
            setAllowPrivate(answer.importAllowPrivateUrls);
            setNotesPublic(answer.annotationsPublic);
            // The header, the page title, the lists and the dates follow at once.
            await reloadInstallation();
            setSaved(true);
          });
        }}
      >
        <fieldset>
          <legend>{t('settings.general')}</legend>
          <p>
            <label htmlFor="setting-administrator_email">{t('settings.administratorEmail')}</label>
            <br />
            <input
              id="setting-administrator_email"
              type="email"
              required
              aria-describedby="setting-administrator_email-info"
              value={email}
              onChange={(event) => {
                edited(setEmail)(event.target.value);
              }}
            />
            <Info id="setting-administrator_email-info">{t('settings.administratorEmailNotYet')}</Info>
          </p>
          <p>
            <label htmlFor="setting-installation_title">{t('settings.installationTitle')}</label>
            <br />
            <input
              id="setting-installation_title"
              required
              pattern=".*\S.*"
              aria-describedby="setting-installation_title-info"
              value={title}
              onChange={(event) => {
                edited(setTitle)(event.target.value);
              }}
            />
            <Info id="setting-installation_title-info">{t('settings.installationTitleInfo')}</Info>
          </p>
          <p>
            <label htmlFor="setting-time_zone">{t('settings.timeZone')}</label>
            <br />
            <select
              id="setting-time_zone"
              required
              aria-describedby="setting-time_zone-info"
              value={timeZone}
              onChange={(event) => {
                edited(setTimeZone)(event.target.value);
              }}
            >
              {timeZones.map((zone) => (
                <option key={zone} value={zone}>
                  {zone}
                </option>
              ))}
            </select>
            <Info id="setting-time_zone-info">{t('settings.timeZoneInfo')}</Info>
          </p>
          <LocaleSelect id="setting-locale" value={locale} onChange={edited(setLocale)} />
          <p>
            <label>
              <input
                type="checkbox"
                checked={versionNotices}
                aria-describedby="setting-version_notifications-info"
                onChange={(event) => {
                  edited(setVersionNotices)(event.target.checked);
                }}
              />
              {t('settings.versionNotifications')}
            </label>
            <Info id="setting-version_notifications-info">{t('settings.versionNotificationsNotYet')}</Info>
          </p>
        </fieldset>
        <fieldset>
          <legend>{t('settings.display')}</legend>
          <p>
            <label htmlFor="setting-pagination_per_page">{t('settings.paginationPerPage')}</label>
            <br />
            <input
              id="setting-pagination_per_page"
              type="number"
              required
              min={1}
              step={1}
              aria-describedby="setting-pagination_per_page-info"
              value={perPage}
              onChange={(event) => {
                edited(setPerPage)(event.target.value);
              }}
            />
            <Info id="setting-pagination_per_page-info">{t('settings.paginationPerPageInfo')}</Info>
          </p>
        </fieldset>
        <fieldset>
          <legend>{t('settings.editing')}</legend>
          <p>
            <label>
              <input
                type="checkbox"
                checked={privateSets}
                aria-describedby="setting-default_to_private_item_sets-info"
                onChange={(event) => {
                  edited(setPrivateSets)(event.target.checked);
                }}
              />
              {t('settings.defaultPrivateItemSets')}
            </label>
            <Info id="setting-default_to_private_item_sets-info">
              {t('settings.defaultPrivateItemSetsInfo')}
            </Info>
          </p>
          <p>
            <label htmlFor="setting-media_alt_text_property">
              {t('settings.mediaAltTextProperty')}
            </label>
            <input
              id="setting-media_alt_text_property"
              aria-describedby="setting-media_alt_text_property-info"
              value={altTextProperty}
              onChange={(event) => {
                edited(setAltTextProperty)(event.target.value);
              }}
            />
            <small id="setting-media_alt_text_property-info">
              {t('settings.mediaAltTextPropertyInfo')}
            </small>
          </p>
        </fieldset>
        <fieldset>
          <legend>{t('settings.search')}</legend>
          <p>
            <label>
              <input
                type="checkbox"
                checked={reindex}
                aria-describedby="setting-index_fulltext_search-info"
                onChange={(event) => {
                  edited(setReindex)(event.target.checked);
                }}
              />
              {t('settings.indexFulltextSearch')}
            </label>
            <Info id="setting-index_fulltext_search-info">{t('settings.indexFulltextSearchInfo')}</Info>
          </p>
        </fieldset>
        <fieldset>
          <legend>{t('settings.notesOnPublicPages')}</legend>
          <p>
            <label>
              <input
                type="checkbox"
                checked={notesPublic}
                aria-describedby="setting-romeka_annotations_public-info"
                onChange={(event) => {
                  edited(setNotesPublic)(event.target.checked);
                }}
              />
              {t('settings.annotationsPublic')}
            </label>
            <br />
            <small id="setting-romeka_annotations_public-info">
              {t('settings.annotationsPublicInfo')}
            </small>
          </p>
        </fieldset>
        <fieldset>
          <legend>{t('settings.security')}</legend>
          <p>
            <label>
              <input
                type="checkbox"
                checked={disabled}
                aria-describedby="setting-disable_file_validation-info"
                onChange={(event) => {
                  edited(setDisabled)(event.target.checked);
                }}
              />
              {t('settings.disableFileValidation')}
            </label>
            <br />
            <small id="setting-disable_file_validation-info">
              {t('settings.disableFileValidationInfo')}
            </small>
          </p>
          <AllowListField
            id="media_type_whitelist"
            label={t('settings.mediaTypeWhitelist')}
            info={t('settings.mediaTypeWhitelistInfo')}
            restoreLabel={t('settings.restoreMediaTypes')}
            restoredNote={t('settings.restoredOnSave')}
            emptyNote={t('settings.emptyListRefusesAll')}
            draft={mediaTypes}
            onChange={edited(setMediaTypes)}
          />
          <AllowListField
            id="extension_whitelist"
            label={t('settings.extensionWhitelist')}
            info={t('settings.extensionWhitelistInfo')}
            restoreLabel={t('settings.restoreExtensions')}
            restoredNote={t('settings.restoredOnSave')}
            emptyNote={t('settings.emptyListRefusesAll')}
            draft={extensions}
            onChange={edited(setExtensions)}
          />
          <p>
            <label>
              <input
                type="checkbox"
                checked={allowPrivate}
                aria-describedby={
                  allowPrivate
                    ? 'setting-import_allow_private_urls-info setting-import_allow_private_urls-note'
                    : 'setting-import_allow_private_urls-info'
                }
                onChange={(event) => {
                  edited(setAllowPrivate)(event.target.checked);
                }}
              />
              {t('settings.allowPrivateUrls')}
            </label>
            <br />
            <small id="setting-import_allow_private_urls-info">
              {t('settings.allowPrivateUrlsInfo')}
            </small>
            {/* The note stays while the switch is on, saved or not: the risk is
                the state, not the edit. */}
            {allowPrivate ? (
              <>
                <br />
                <small id="setting-import_allow_private_urls-note">
                  {t('settings.allowPrivateUrlsWarning')}
                </small>
              </>
            ) : null}
          </p>
        </fieldset>
        <button type="submit" disabled={saveState.status === 'saving'}>
          {t('action.save')}
        </button>
      </form>
      {saveState.status === 'error' ? <SaveStatus state={saveState} /> : null}
      {saved ? <p role="status">{t('settings.saved')}</p> : null}
      {saved && indexJob !== null ? (
        <p role="status">
          {t('settings.indexStarted', { job: indexJob })}{' '}
          <a href={`#/jobs/${String(indexJob)}`}>{t('settings.indexStartedLink')}</a>
        </p>
      ) : null}
    </section>
  );
};

interface AllowListFieldProps {
  readonly id: string;
  readonly label: string;
  readonly info: string;
  readonly restoreLabel: string;
  readonly restoredNote: string;
  readonly emptyNote: string;
  readonly draft: ListDraft;
  readonly onChange: (draft: ListDraft) => void;
}

/**
 * `Form\Element\RestoreTextarea`: a comma-separated list and its restore
 * button. Under the list a note says what saving will do when that differs
 * from what the text shows: a pending restore, or an empty list, which
 * refuses every file and which upstream saves without a word.
 */
const AllowListField = ({
  id,
  label,
  info,
  restoreLabel,
  restoredNote,
  emptyNote,
  draft,
  onChange,
}: AllowListFieldProps): ReactElement => {
  const names = listNames(draft);
  const note = names === null ? restoredNote : names.length === 0 ? emptyNote : null;
  const described = [`setting-${id}-info`, ...(note === null ? [] : [`setting-${id}-note`])];
  return (
    <p>
      <label htmlFor={`setting-${id}`}>{label}</label>
      <br />
      <small id={`setting-${id}-info`}>{info}</small>
      <br />
      <textarea
        id={`setting-${id}`}
        rows={4}
        aria-describedby={described.join(' ')}
        value={draft ?? ''}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {note === null ? null : (
        <>
          <br />
          <small id={`setting-${id}-note`}>{note}</small>
        </>
      )}
      <br />
      <button
        type="button"
        onClick={() => {
          onChange(null);
        }}
      >
        {restoreLabel}
      </button>
    </p>
  );
};
