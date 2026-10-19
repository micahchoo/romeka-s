/**
 * The controls for one navigation link, chosen by its type: a page link picks
 * one of the site's pages, a URL link takes an address, a browse link takes an
 * item query (the page editor's builder), an item set browse link a query
 * string. A link type the port does not know keeps its stored data untouched.
 */
import type { ReactElement } from 'react';
import type { PageId } from '../../api/types';
import { CheckboxField, OptionSelect, TextField } from '../../app/fields';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../i18n';
import { ItemQueryField } from './editor/fields/ItemQueryField';
import { NAVIGATION_TYPES, isNavigationType } from './navigationTree';
import type { LinkPatch, NavLink } from './navigationTree';
import { sites } from '../../i18n/namespaces/sites';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof sites>;

export interface PageOption {
  readonly id: PageId;
  readonly title: string;
}

export const TYPE_LABELS: Readonly<Record<(typeof NAVIGATION_TYPES)[number], MessageId>> = {
  page: 'navigation.typePage',
  url: 'navigation.typeUrl',
  browse: 'navigation.typeBrowse',
  browseItemSets: 'navigation.typeBrowseItemSets',
};

export const typeLabel = (type: string, t: Translator<MessageId>['t']): string =>
  isNavigationType(type) ? t(TYPE_LABELS[type]) : type;

/**
 * What the public menu shows for a link (`nav.rs::label_for`): the user's
 * label, else the page title, else the link type's name.
 */
export const linkLabel = (
  link: NavLink,
  pages: readonly PageOption[],
  t: Translator<MessageId>['t'],
): string => {
  const label = link.label.trim();
  if (label !== '') {
    return label;
  }
  if (link.type === 'page') {
    return pages.find((page) => page.id === link.pageId)?.title ?? t('siteHub.missingPage');
  }
  return typeLabel(link.type, t);
};

export const NavigationLinkFields = ({
  link,
  pages,
  onChange,
}: {
  readonly link: NavLink;
  readonly pages: readonly PageOption[];
  readonly onChange: (patch: LinkPatch) => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const id = (suffix: string): string => `nav-${String(link.key)}-${suffix}`;
  const known = isNavigationType(link.type);
  return (
    <div className="nav-link-fields" id={id('fields')}>
      <p>
        <label htmlFor={id('type')}>{t('navigation.type')}</label>
        <select
          id={id('type')}
          value={link.type}
          onChange={(event) => {
            onChange({ type: event.target.value });
          }}
        >
          {known ? null : <option value={link.type}>{link.type}</option>}
          {NAVIGATION_TYPES.map((type) => (
            <option key={type} value={type}>
              {t(TYPE_LABELS[type])}
            </option>
          ))}
        </select>
      </p>
      {link.type === 'page' ? (
        <OptionSelect
          id={id('page')}
          label={t('navigation.page')}
          options={pages.map((page) => ({ id: page.id, label: page.title }))}
          value={link.pageId}
          onChange={(pageId) => {
            onChange({ pageId });
          }}
          emptyLabel={t('siteHub.choosePage')}
        />
      ) : null}
      {link.type === 'url' ? (
        <>
          <TextField
            id={id('url')}
            label={t('navigation.url')}
            value={link.url}
            onChange={(url) => {
              onChange({ url });
            }}
          />
          <CheckboxField
            id={id('target-blank')}
            label={t('navigation.targetBlank')}
            checked={link.targetBlank}
            onChange={(targetBlank) => {
              onChange({ targetBlank });
            }}
          />
        </>
      ) : null}
      {link.type === 'browse' ? (
        <ItemQueryField
          id={id('query')}
          label={t('navigation.query')}
          value={link.query}
          onChange={(query) => {
            onChange({ query });
          }}
        />
      ) : null}
      {link.type === 'browseItemSets' ? (
        <TextField
          id={id('query')}
          label={t('navigation.query')}
          value={link.query}
          onChange={(query) => {
            onChange({ query });
          }}
        />
      ) : null}
      {known ? (
        <>
          <TextField
            id={id('label')}
            label={t('navigation.label')}
            value={link.label}
            onChange={(label) => {
              onChange({ label });
            }}
          />
          {link.type === 'url' ? null : (
            <p className="field-note">
              {t('siteHub.labelNote', { label: linkLabel({ ...link, label: '' }, pages, t) })}
            </p>
          )}
        </>
      ) : (
        <p className="field-note">{t('siteHub.unknownLinkType')}</p>
      )}
    </div>
  );
};
