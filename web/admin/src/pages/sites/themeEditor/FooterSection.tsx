/**
 * The Footer section: up to three HTML columns, the footer's links, a contact
 * block, a licence and the "Powered by Romeka S" switch. The links use the
 * navigation link editor; until the curator chooses links of their own, the
 * footer repeats the header menu's top level.
 */
import type { ReactElement } from 'react';
import { api } from '../../../api/client';
import type { SiteId } from '../../../api/types';
import { CheckboxField } from '../../../app/fields';
import { useAsync } from '../../../app/useAsync';
import { HtmlEditor } from '../editor/fields/HtmlEditor';
import { NavigationLinkFields, linkLabel } from '../NavigationLinkFields';
import type { PageOption } from '../NavigationLinkFields';
import { navigationReducer } from '../navigationTree';
import type { NavLink, NavigationAction } from '../navigationTree';
import { useTranslator } from '../../../i18n';
import { sites } from '../../../i18n/namespaces/sites';
import { choiceKey } from './controls';
import type { FooterDraft } from './draft';
import { useThemeText } from './messages';
import { settingFor } from './registry';

const NO_PAGES: readonly PageOption[] = [];

/** The most columns the schema allows. */
const maxColumns = (): number => {
  const setting = settingFor('footer_columns');
  return setting?.kind === 'htmlColumns' ? setting.max : 3;
};

const licences = (): readonly string[] => {
  const setting = settingFor('footer_licence');
  return setting?.kind === 'choice' ? setting.choices : ['none'];
};

const FooterLinks = ({
  links,
  pages,
  onChange,
}: {
  readonly links: readonly NavLink[];
  readonly pages: readonly PageOption[];
  readonly onChange: (links: readonly NavLink[]) => void;
}): ReactElement => {
  const tt = useThemeText();
  const { t } = useTranslator(sites);
  const apply = (action: NavigationAction): void => {
    onChange(navigationReducer(links, action));
  };
  return (
    <div className="theme-footer-links">
      {links.length === 0 ? <p className="field-note">{tt('theme.footerNoLinks')}</p> : null}
      <ol>
        {links.map((link, index) => {
          const label = linkLabel(link, pages, t);
          const previous = links[index - 1];
          const next = links[index + 1];
          return (
            <li key={link.key}>
              <NavigationLinkFields
                link={link}
                pages={pages}
                onChange={(patch) => {
                  apply({ type: 'patch', key: link.key, patch });
                }}
              />
              <p className="theme-footer-link-actions">
                <button
                  type="button"
                  disabled={previous === undefined}
                  aria-label={tt('theme.footerLinkUp', { label })}
                  onClick={() => {
                    if (previous !== undefined) {
                      apply({ type: 'move', key: link.key, target: previous.key, place: 'before' });
                    }
                  }}
                >
                  ↑
                </button>
                <button
                  type="button"
                  disabled={next === undefined}
                  aria-label={tt('theme.footerLinkDown', { label })}
                  onClick={() => {
                    if (next !== undefined) {
                      apply({ type: 'move', key: link.key, target: next.key, place: 'after' });
                    }
                  }}
                >
                  ↓
                </button>
                <button
                  type="button"
                  onClick={() => {
                    apply({ type: 'remove', key: link.key });
                  }}
                >
                  {tt('theme.footerLinkRemove', { label })}
                </button>
              </p>
            </li>
          );
        })}
      </ol>
      <p>
        <button
          type="button"
          onClick={() => {
            apply({ type: 'add', parent: null, linkType: 'url' });
          }}
        >
          {tt('theme.footerLinkAdd')}
        </button>
      </p>
    </div>
  );
};

export const FooterSection = ({
  siteId,
  footer,
  onChange,
}: {
  readonly siteId: SiteId;
  readonly footer: FooterDraft;
  readonly onChange: (footer: FooterDraft) => void;
}): ReactElement => {
  const tt = useThemeText();
  const pages = useAsync(async () => {
    const page = await api.listSitePages({ siteId, perPage: 200 });
    return page.items.map((item): PageOption => ({ id: item.id, title: item.title }));
  }, [siteId]);
  const max = maxColumns();
  const help = (key: string): ReactElement | null => {
    const setting = settingFor(key);
    return setting === undefined ? null : <p className="field-note theme-help">{tt(setting.helpKey)}</p>;
  };
  const setColumn = (index: number, value: string): void => {
    onChange({ ...footer, columns: footer.columns.map((column, at) => (at === index ? value : column)) });
  };
  return (
    <fieldset className="theme-section theme-footer">
      <legend>{tt('theme.footer')}</legend>
      <p className="field-note">{tt('theme.footerHelp')}</p>
      <h4>{tt('themeSetting.footerColumns')}</h4>
      {help('footer_columns')}
      {footer.columns.map((column, index) => (
        <div className="theme-footer-column" key={String(index)}>
          <HtmlEditor
            id={`theme-footer-column-${String(index + 1)}`}
            label={tt('theme.footerColumn', { number: index + 1 })}
            value={column}
            onChange={(value) => {
              setColumn(index, value);
            }}
          />
          <p>
            <button
              type="button"
              onClick={() => {
                onChange({ ...footer, columns: footer.columns.filter((_, at) => at !== index) });
              }}
            >
              {tt('theme.footerColumnRemove', { number: index + 1 })}
            </button>
          </p>
        </div>
      ))}
      {footer.columns.length < max ? (
        <p>
          <button
            type="button"
            onClick={() => {
              onChange({ ...footer, columns: [...footer.columns, ''] });
            }}
          >
            {tt('theme.footerColumnAdd')}
          </button>
        </p>
      ) : null}
      <h4>{tt('themeSetting.footerLinks')}</h4>
      {help('footer_links')}
      <CheckboxField
        id="theme-footer-use-menu"
        label={tt('theme.footerUseMenu')}
        checked={footer.links === null}
        onChange={(useMenu) => {
          onChange({ ...footer, links: useMenu ? null : [] });
        }}
      />
      {footer.links === null ? null : (
        <FooterLinks
          links={footer.links}
          pages={pages.status === 'ready' ? pages.data : NO_PAGES}
          onChange={(links) => {
            onChange({ ...footer, links });
          }}
        />
      )}
      <HtmlEditor
        id="theme-setting-footer_contact"
        label={tt('themeSetting.footerContact')}
        value={footer.contact}
        onChange={(contact) => {
          onChange({ ...footer, contact });
        }}
      />
      {help('footer_contact')}
      <p>
        <label htmlFor="theme-setting-footer_licence">{tt('themeSetting.footerLicence')}</label>
        <select
          id="theme-setting-footer_licence"
          value={footer.licence}
          onChange={(event) => {
            onChange({ ...footer, licence: event.target.value });
          }}
        >
          {licences().map((licence) => {
            const key = choiceKey(licence);
            return (
              <option key={licence} value={licence}>
                {key === null ? licence : tt(key)}
              </option>
            );
          })}
        </select>
      </p>
      {help('footer_licence')}
      <CheckboxField
        id="theme-setting-show_powered_by"
        label={tt('themeSetting.showPoweredBy')}
        checked={footer.poweredBy}
        onChange={(poweredBy) => {
          onChange({ ...footer, poweredBy });
        }}
      />
      {help('show_powered_by')}
    </fieldset>
  );
};
