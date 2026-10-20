/**
 * `SiteAdmin/IndexController::addAction`: the new site form. It asks only for
 * what a new site needs; the summary, owner and the rest live in the site's
 * Settings tab once it exists.
 *
 * A new site holds no items and does not sync: the curator chooses its items
 * and item sets on its Items tab, the one place that changes them.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { CheckboxField, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { useSave } from '../../app/save';
import { useTranslator } from '../../i18n';
import { Breadcrumb } from './SiteFrame';
import { ThemeSelect, slugFromTitle } from './siteInfo';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

export const SiteCreatePage = (): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [title, setTitle] = useState('');
  /** `null` while the slug follows the title; the user's text once they edit it. */
  const [typedSlug, setTypedSlug] = useState<string | null>(null);
  const [theme, setTheme] = useState('default');
  const [isPublic, setIsPublic] = useState(true);
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();
  const slug = typedSlug ?? slugFromTitle(title);

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (title.trim() === '') {
      setFormError(t('sites.errorNoTitle'));
      return;
    }
    setFormError(null);
    save(async () => {
      const created = await api.createSite({
        title: title.trim(),
        slug: slug.trim(),
        theme,
        summary: null,
        itemPool: null,
        isPublic,
        assignNewItems: false,
        owner: null,
      });
      window.location.hash = `#/sites/${String(created.id)}`;
    });
  };

  return (
    <div className="site-frame">
      <Breadcrumb
        crumbs={[
          { label: t('page.sites'), href: '#/sites' },
          { label: t('sites.add'), href: null },
        ]}
      />
      <header className="site-frame-header">
        <h2>{t('sites.add')}</h2>
      </header>
      <form onSubmit={submit}>
        <fieldset>
          <legend>{t('sites.legend')}</legend>
          <TextField id="site-title" label={t('sites.title')} value={title} onChange={setTitle} />
          <TextField id="site-slug" label={t('sites.slug')} value={slug} onChange={setTypedSlug} />
          <p className="field-note">{t('siteHub.slugNote', { url: `/s/${slug}` })}</p>
          <ThemeSelect value={theme} onChange={setTheme} />
          <CheckboxField
            id="site-public"
            label={t('common.public')}
            checked={isPublic}
            onChange={setIsPublic}
          />
        </fieldset>
        {formError === null ? null : <p role="alert">{formError}</p>}
        <SaveStatus state={saveState} />
        <SubmitRow label={t('sites.add')} busy={saveState.status === 'saving'} />
      </form>
    </div>
  );
};
