/**
 * The site's own members (`SiteForm`: title, slug, theme, summary, owner,
 * visibility), as the Settings tab and the Add site form edit them.
 * `encodeSiteInfo` (`api/write.ts`) is the body they save with. Upstream's
 * "assign new items" lives on the site's Items page as "Keep this site in sync".
 */
import type { ReactElement } from 'react';
import type { Site, User, UserId } from '../../api/types';
import { CheckboxField, OptionSelect, TextField } from '../../app/fields';
import { userOptions } from '../../app/options';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

/**
 * The bundled theme (`crates/omeka-site/src/theme/default.rs`). No API endpoint
 * lists themes, so the select offers the port's one theme plus the stored value.
 */
const THEME_OPTIONS = ['default'];

export interface SiteInfo {
  readonly title: string;
  readonly slug: string;
  readonly theme: string;
  readonly summary: string;
  readonly owner: UserId | null;
  readonly isPublic: boolean;
}

export const siteInfoOf = (site: Site): SiteInfo => ({
  title: site.title,
  slug: site.slug,
  theme: site.theme,
  summary: site.summary ?? '',
  owner: site.owner?.id ?? null,
  isPublic: site.isPublic,
});

/**
 * The slug the server would derive from a title (`SiteSlugTrait::slugify`):
 * accents dropped, lower case, every other run of characters one hyphen.
 */
export const slugFromTitle = (title: string): string =>
  title
    .normalize('NFD')
    .replace(/\p{Mn}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

export const ThemeSelect = ({
  value,
  onChange,
}: {
  readonly value: string;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const options = THEME_OPTIONS.includes(value) ? THEME_OPTIONS : [...THEME_OPTIONS, value];
  return (
    <p>
      <label htmlFor="site-theme">{t('sites.theme')}</label>
      <select
        id="site-theme"
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option === 'default' ? t('sites.defaultThemeLabel') : option}
          </option>
        ))}
      </select>
    </p>
  );
};

export const SiteInfoFields = ({
  info,
  users,
  onChange,
}: {
  readonly info: SiteInfo;
  readonly users: readonly User[];
  readonly onChange: (info: SiteInfo) => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const set = (patch: Partial<SiteInfo>): void => {
    onChange({ ...info, ...patch });
  };
  return (
    <fieldset>
      <legend>{t('sites.legend')}</legend>
      <TextField
        id="site-title"
        label={t('sites.title')}
        value={info.title}
        onChange={(title) => {
          set({ title });
        }}
      />
      <TextField
        id="site-slug"
        label={t('sites.slug')}
        value={info.slug}
        onChange={(slug) => {
          set({ slug });
        }}
      />
      <ThemeSelect
        value={info.theme}
        onChange={(theme) => {
          set({ theme });
        }}
      />
      <TextField
        id="site-summary"
        label={t('sites.summary')}
        value={info.summary}
        onChange={(summary) => {
          set({ summary });
        }}
      />
      <OptionSelect
        id="site-owner"
        label={t('field.owner')}
        options={userOptions(users)}
        value={info.owner}
        onChange={(owner) => {
          set({ owner });
        }}
        emptyLabel={t('common.noOwner')}
      />
      <CheckboxField
        id="site-public"
        label={t('common.public')}
        checked={info.isPublic}
        onChange={(isPublic) => {
          set({ isPublic });
        }}
      />
    </fieldset>
  );
};
