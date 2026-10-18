/**
 * The `pageRef` field control: a select of the site's pages, read from
 * `/api/site_pages?site_id=...`. A stored id the fetched page does not name
 * keeps its own option, so opening a block never silently retargets its link.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { SiteId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';

export interface PageSelectProps {
  readonly id: string;
  readonly label: string;
  readonly siteId: SiteId;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly required?: boolean;
}

export const PageSelect = ({
  id,
  label,
  siteId,
  value,
  onChange,
  required = false,
}: PageSelectProps): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => api.listSitePages({ siteId, perPage: 200 }), [siteId]);
  const pages = state.status === 'ready' ? state.data.items : [];
  const options = pages.map((page) => ({
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
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
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
