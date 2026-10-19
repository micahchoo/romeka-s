import type { ReactElement } from 'react';
import { LOCALES, parseLocale, useTranslator } from '../i18n';
import { admin } from '../i18n/namespaces/admin';

/**
 * A stored `locale` setting as the select's value: `''` for none, which is the
 * Default option, and otherwise the shipped locale it narrows to. Upstream
 * stores region tags such as `en_US`; `parseLocale` drops the region.
 */
export const localeValue = (stored: string | null): string =>
  stored === null || stored === '' ? '' : parseLocale(stored);

/**
 * The `locale` field of `SettingForm` and `UserForm`: `LocaleSelectFactory`'s
 * "Default" empty option, then every shipped locale by its own name. The value
 * is the bare tag the SPA ships, which is what the setting stores.
 */
export const LocaleSelect = ({
  id,
  value,
  onChange,
}: {
  readonly id: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const { t } = useTranslator(admin);
  return (
    <p>
      <label htmlFor={id}>{t('settings.locale')}</label>
      <select
        id={id}
        value={value}
        onChange={(event) => {
          onChange(localeValue(event.target.value));
        }}
      >
        <option value="">{t('settings.localeDefault')}</option>
        {LOCALES.map((option) => (
          <option key={option} value={option}>
            {t(`locale.${option}`)}
          </option>
        ))}
      </select>
    </p>
  );
};
