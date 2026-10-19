/**
 * The `link` field control: one target, a page of the site or a web address,
 * stored under two keys (`cta_page`, `cta_url`). The renderer prefers the page
 * when both are set (`hero.rs::href`), so this control keeps only one: picking
 * a kind clears the other key.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import type { JsonValue, SiteId, SitePage } from '../../../../api/types';
import { useEditorText } from '../messages';
import { Help, helpId } from '../Labelled';
import { PageSelect } from './PageSelect';
import './controls.css';

type LinkKind = 'page' | 'url';

const textOf = (value: JsonValue | undefined): string =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';

export interface LinkFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly pageKey: string;
  readonly urlKey: string;
  readonly record: Readonly<Record<string, JsonValue>>;
  /** Writes several stored keys as one edit. */
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
  readonly siteId: SiteId;
  readonly pages?: readonly SitePage[];
}

export const LinkField = ({
  id,
  label,
  help,
  pageKey,
  urlKey,
  record,
  onPatch,
  siteId,
  pages,
}: LinkFieldProps): ReactElement => {
  const t = useEditorText();
  const page = textOf(record[pageKey]);
  const url = textOf(record[urlKey]);
  const [kind, setKind] = useState<LinkKind>(page === '' && url !== '' ? 'url' : 'page');
  const choose = (next: LinkKind): void => {
    setKind(next);
    const cleared = next === 'page' ? urlKey : pageKey;
    if (textOf(record[cleared]) !== '') {
      onPatch({ [cleared]: '' });
    }
  };
  return (
    <fieldset className="block-field link-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      <div className="link-kinds">
        {(['page', 'url'] as const).map((option) => (
          <label key={option} className="block-check">
            <input
              type="radio"
              name={`${id}-kind`}
              value={option}
              checked={kind === option}
              onChange={() => {
                choose(option);
              }}
            />
            {t(option === 'page' ? 'link.page' : 'link.url')}
          </label>
        ))}
      </div>
      {kind === 'page' ? (
        <PageSelect
          id={id}
          label={t('link.page')}
          hideLabel
          siteId={siteId}
          value={page}
          onChange={(value) => {
            onPatch({ [pageKey]: value });
          }}
          {...(pages === undefined ? {} : { pages })}
        />
      ) : (
        <input
          id={id}
          type="url"
          aria-label={t('link.url')}
          placeholder={t('fieldExample.url')}
          value={url}
          onChange={(event) => {
            onPatch({ [urlKey]: event.target.value });
          }}
        />
      )}
      <Help id={id} text={help} />
    </fieldset>
  );
};
