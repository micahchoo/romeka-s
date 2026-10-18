/**
 * The site add and edit form (`SiteAdmin/IndexController::addAction` and
 * `::editAction`). One component serves both; a `null` site is a create. The
 * navigation and item pool members are edited as JSON, as upstream's site form
 * does not flatten them into scalar fields.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { parseJsonArrayText, parseJsonObjectOrArrayText } from '../../api/parse';
import type { PageId, Site, SiteWrite, User, UserId } from '../../api/types';
import { CheckboxField, OptionSelect, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { formatJson } from '../../app/json';
import { userOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';

/**
 * The navigation `SiteAdapter::hydrate` writes when a create sends none
 * (`default_navigation` in `crates/omeka-api/src/write/sites.rs`). The label is
 * stored data, matching upstream, so it is not a catalogue message.
 */
const DEFAULT_NAVIGATION = [
  { type: 'browse', data: { label: 'Browse', query: '' }, links: [] },
];

/**
 * The bundled theme (`crates/omeka-site/src/theme/default.rs`). No API endpoint
 * lists themes, so the select offers the port's one theme plus the stored value.
 */
const THEME_OPTIONS = ['default'];

/** A JSON field parse that reports its error text instead of throwing. */
const tryParse = <T,>(
  label: string,
  parse: (text: string) => T,
  text: string,
): { readonly value: T } | { readonly error: string } => {
  try {
    return { value: parse(text) };
  } catch (error) {
    return { error: `${label}: ${toError(error).message}` };
  }
};

export interface PageSummary {
  readonly id: PageId;
  readonly title: string | null;
  /** `SitePage::isPublic`, `null` when the read failed. */
  readonly isPublic: boolean | null;
}

export interface SiteEditorProps {
  readonly site: Site | null;
  readonly users: readonly User[];
  readonly pages: readonly PageSummary[];
}

const SiteEditor = ({ site, users, pages }: SiteEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [title, setTitle] = useState(site?.title ?? '');
  const [slug, setSlug] = useState(site?.slug ?? '');
  const [theme, setTheme] = useState(site?.theme ?? 'default');
  const [summary, setSummary] = useState(site?.summary ?? '');
  const [navigation, setNavigation] = useState(() =>
    formatJson(site?.navigation ?? DEFAULT_NAVIGATION),
  );
  const [itemPool, setItemPool] = useState(() => formatJson(site?.itemPool ?? []));
  const [isPublic, setIsPublic] = useState(site?.isPublic ?? true);
  const [assignNewItems, setAssignNewItems] = useState(site?.assignNewItems ?? true);
  const [owner, setOwner] = useState<UserId | null>(site?.owner?.id ?? null);
  const [homepage, setHomepage] = useState<PageId | null>(site?.homepage?.id ?? null);
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const themeOptions =
    site === null || THEME_OPTIONS.includes(theme) ? THEME_OPTIONS : [...THEME_OPTIONS, theme];

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (title.trim() === '') {
      setFormError(t('sites.errorNoTitle'));
      return;
    }
    const navigationValue = tryParse(t('sites.navigation'), parseJsonArrayText, navigation);
    if ('error' in navigationValue) {
      setFormError(navigationValue.error);
      return;
    }
    const itemPoolValue = tryParse(t('sites.itemPool'), parseJsonObjectOrArrayText, itemPool);
    if ('error' in itemPoolValue) {
      setFormError(itemPoolValue.error);
      return;
    }
    setFormError(null);
    const trimmedSummary = summary.trim();
    const write: SiteWrite = {
      title: title.trim(),
      slug: slug.trim(),
      theme,
      summary: trimmedSummary === '' ? null : trimmedSummary,
      navigation: navigationValue.value,
      homepage,
      itemPool: itemPoolValue.value,
      isPublic,
      assignNewItems,
      owner,
    };
    save(async () => {
      if (site === null) {
        const created = await api.createSite(write);
        window.location.hash = `#/sites/${String(created.id)}`;
      } else {
        await api.updateSite(site.id, write);
      }
    });
  };

  return (
    <form onSubmit={submit}>
      <fieldset>
        <legend>{t('sites.legend')}</legend>
        <TextField id="site-title" label={t('sites.title')} value={title} onChange={setTitle} />
        <TextField id="site-slug" label={t('sites.slug')} value={slug} onChange={setSlug} />
        <p>
          <label htmlFor="site-theme">{t('sites.theme')}</label>
          <select
            id="site-theme"
            value={theme}
            onChange={(event) => {
              setTheme(event.target.value);
            }}
          >
            {themeOptions.map((option) => (
              <option key={option} value={option}>
                {option === 'default' ? t('sites.defaultThemeLabel') : option}
              </option>
            ))}
          </select>
        </p>
        <TextField id="site-summary" label={t('sites.summary')} value={summary} onChange={setSummary} />
        <OptionSelect
          id="site-owner"
          label={t('field.owner')}
          options={userOptions(users)}
          value={owner}
          onChange={setOwner}
          emptyLabel={t('common.noOwner')}
        />
        <CheckboxField
          id="site-public"
          label={t('common.public')}
          checked={isPublic}
          onChange={setIsPublic}
        />
        <CheckboxField
          id="site-assign-new-items"
          label={t('sites.assignNewItems')}
          checked={assignNewItems}
          onChange={setAssignNewItems}
        />
      </fieldset>
      <fieldset>
        <legend>{t('sites.contentLegend')}</legend>
        {site === null ? null : (
          <OptionSelect
            id="site-homepage"
            label={t('sites.homepage')}
            options={pages.map((page) => ({
              id: page.id,
              label: page.title ?? t('common.pageFallback', { id: page.id }),
            }))}
            value={homepage}
            onChange={setHomepage}
            emptyLabel={t('common.none')}
          />
        )}
        <p>
          <label htmlFor="site-navigation">{t('sites.navigation')}</label>
          <textarea
            id="site-navigation"
            rows={6}
            value={navigation}
            onChange={(event) => {
              setNavigation(event.target.value);
            }}
          />
        </p>
        <p>
          <label htmlFor="site-item-pool">{t('sites.itemPool')}</label>
          <textarea
            id="site-item-pool"
            rows={4}
            value={itemPool}
            onChange={(event) => {
              setItemPool(event.target.value);
            }}
          />
        </p>
      </fieldset>
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow
        label={site === null ? t('sites.add') : t('action.save')}
        busy={saveState.status === 'saving'}
      />
    </form>
  );
};

/** The add form: no stored site and no page list to choose a homepage from. */
export const SiteCreateForm = ({ users }: { readonly users: readonly User[] }): ReactElement => (
  <SiteEditor site={null} users={users} pages={[]} />
);

/** The edit form for a stored site. */
export const SiteEditForm = ({ site, users, pages }: SiteEditorProps): ReactElement => (
  <SiteEditor site={site} users={users} pages={pages} />
);
