import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { GlobalSettings } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus } from '../../app/fields';
import { LocaleSelect, localeValue } from '../../app/locale-select';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof admin>;

interface SettingField {
  readonly id: string;
  readonly labelKey: MessageId;
  readonly kind: 'text' | 'checkbox';
}

/** The global settings `Admin\SettingForm` exposes, in its group order. */
const SETTING_FIELDS: readonly SettingField[] = [
  { id: 'installation_title', labelKey: 'settings.installationTitle', kind: 'text' },
  { id: 'administrator_email', labelKey: 'settings.administratorEmail', kind: 'text' },
  { id: 'time_zone', labelKey: 'settings.timeZone', kind: 'text' },
  { id: 'pagination_per_page', labelKey: 'settings.paginationPerPage', kind: 'text' },
  {
    id: 'default_to_private_item_sets',
    labelKey: 'settings.defaultPrivateItemSets',
    kind: 'checkbox',
  },
  { id: 'index_fulltext_search', labelKey: 'settings.indexFulltextSearch', kind: 'checkbox' },
  { id: 'version_notifications', labelKey: 'settings.versionNotifications', kind: 'checkbox' },
];

const EMPTY_VALUES: Readonly<Record<string, string>> = Object.fromEntries(
  SETTING_FIELDS.map((field) => [field.id, field.kind === 'checkbox' ? 'false' : '']),
);

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

/**
 * `Admin/SettingController::browseAction`. The port's `/api/settings` stores
 * the locale, the alt text property, the three file validation settings, the
 * private URL switch and the notes switch (`annotationsPublic`), so those are
 * saved; every other field keeps its edits in page state and does not persist
 * them.
 */
export const SettingsPage = (): ReactElement => {
  const { t } = useTranslator(common, admin);
  const stored = useAsync(() => api.getSettings(), []);
  if (stored.status === 'loading') {
    return <p role="status">{t('loading.page')}</p>;
  }
  // A caller other than a global admin is refused, as upstream's ACL refuses
  // them the settings controller.
  if (stored.status === 'error') {
    return <ErrorNotice error={stored.error} />;
  }
  return <SettingsForm stored={stored.data} />;
};

const SettingsForm = ({ stored }: { readonly stored: GlobalSettings }): ReactElement => {
  const { t } = useTranslator(common, admin);
  const [values, setValues] = useState<Readonly<Record<string, string>>>(EMPTY_VALUES);
  const [saved, setSaved] = useState(false);
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
  const setValue = (id: string, value: string): void => {
    setSaved(false);
    setValues((previous) => ({ ...previous, [id]: value }));
  };

  return (
    <section>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          setSaved(false);
          save(async () => {
            const answer = await api.updateSettings({
              locale,
              mediaAltTextProperty: altTextProperty,
              mediaTypeWhitelist: listNames(mediaTypes),
              extensionWhitelist: listNames(extensions),
              disableFileValidation: disabled,
              importAllowPrivateUrls: allowPrivate,
              annotationsPublic: notesPublic,
            });
            setMediaTypes(listText(answer.mediaTypeWhitelist));
            setExtensions(listText(answer.extensionWhitelist));
            setAltTextProperty(answer.mediaAltTextProperty ?? '');
            setDisabled(answer.disableFileValidation);
            setAllowPrivate(answer.importAllowPrivateUrls);
            setNotesPublic(answer.annotationsPublic);
            setSaved(true);
          });
        }}
      >
        <LocaleSelect id="setting-locale" value={locale} onChange={edited(setLocale)} />
        {SETTING_FIELDS.map((field) =>
          field.kind === 'text' ? (
            <p key={field.id}>
              <label htmlFor={`setting-${field.id}`}>{t(field.labelKey)}</label>
              <input
                id={`setting-${field.id}`}
                value={values[field.id] ?? ''}
                onChange={(event) => {
                  setValue(field.id, event.target.value);
                }}
              />
            </p>
          ) : (
            <p key={field.id}>
              <label>
                <input
                  type="checkbox"
                  checked={values[field.id] === 'true'}
                  onChange={(event) => {
                    setValue(field.id, event.target.checked ? 'true' : 'false');
                  }}
                />
                {t(field.labelKey)}
              </label>
            </p>
          ),
        )}
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
      <p>{t('settings.note')}</p>
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
