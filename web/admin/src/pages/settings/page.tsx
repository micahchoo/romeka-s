import type { ReactElement } from 'react';
import { useState } from 'react';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';

interface SettingField {
  readonly id: string;
  readonly labelKey: MessageKey;
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
 * `Admin/SettingController::browseAction`. The port serves no settings
 * endpoint, so this form keeps edits in page state and does not persist them.
 */
export const SettingsPage = (): ReactElement => {
  const { t } = useTranslator();
  const [values, setValues] = useState<Readonly<Record<string, string>>>(EMPTY_VALUES);
  const [saved, setSaved] = useState(false);

  const setValue = (id: string, value: string): void => {
    setSaved(false);
    setValues((previous) => ({ ...previous, [id]: value }));
  };

  return (
    <section>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          setSaved(true);
        }}
      >
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
        <button type="submit">{t('action.save')}</button>
      </form>
      {saved ? <p role="status">{t('settings.saved')}</p> : null}
      <p>{t('settings.note')}</p>
    </section>
  );
};
