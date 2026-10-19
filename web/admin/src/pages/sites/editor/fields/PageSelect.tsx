/**
 * The `pageRef` field control: a select of the site's pages, read from
 * `/api/site_pages?site_id=...`. A stored id the fetched page does not name
 * keeps its own option, so opening a block never silently retargets its link.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { SiteId, SitePage } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import { editor } from '../../../../i18n/namespaces/editor';

export interface PageSelectProps {
  readonly id: string;
  readonly label: string;
  readonly siteId: SiteId;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly required?: boolean;
  /** The site's pages when the editor already loaded them; the control then makes no request. */
  readonly pages?: readonly SitePage[];
  /** Names the select without a visible label, inside a group that shows one. */
  readonly hideLabel?: boolean;
}

export const PageSelect = ({
  id,
  label,
  siteId,
  value,
  onChange,
  required = false,
  pages,
  hideLabel = false,
}: PageSelectProps): ReactElement => {
  const { t } = useTranslator(common, sites, editor);
  const state = useAsync(
    async (): Promise<readonly SitePage[]> =>
      pages ?? (await api.listSitePages({ siteId, perPage: 200 })).items,
    [siteId, pages],
  );
  const items = pages ?? (state.status === 'ready' ? state.data : []);
  const options = items.map((page) => ({
    id: String(page.id),
    label: page.title === '' ? t('common.untitled') : page.title,
  }));
  const named = options.some((option) => option.id === value);
  const all =
    value !== '' && !named
      ? [{ id: value, label: t('common.pageFallback', { id: value }) }, ...options]
      : options;
  return (
    <div className="block-field">
      {hideLabel ? null : <label htmlFor={id}>{label}</label>}
      <select
        id={id}
        {...(hideLabel ? { 'aria-label': label } : {})}
        value={value}
        required={required}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        <option value="">{t('common.none')}</option>
        {all.map((option) => (
          <option key={option.id} value={option.id}>
            {option.label}
          </option>
        ))}
      </select>
      {state.status === 'error' ? (
        <p className="field-error" role="alert">
          {t('field.pagesError')}
        </p>
      ) : null}
    </div>
  );
};
