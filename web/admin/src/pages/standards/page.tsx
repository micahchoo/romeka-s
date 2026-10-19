/**
 * Advanced > Standards: the vocabularies, properties and classes that fields
 * and item types are saved as, one tab each. Each tab is a route
 * (`#/standards/<tab>/<id>`), and upstream's own routes for the three reach the
 * same tabs through the router.
 */
import type { ComponentType, ReactElement } from 'react';
import type { MessageKey } from '../../i18n';
import { useTranslator } from '../../i18n';
import { useStandardsText } from './messages';
import { PropertiesPage } from '../properties/page';
import { ResourceClassesPage } from '../resource-classes/page';
import { VocabulariesPage } from '../vocabularies/page';
import { isStandardsTab, STANDARDS_TABS, standardsHref } from './routes';
import type { StandardsTab } from './routes';

interface Tab {
  readonly labelKey: MessageKey;
  readonly element: ComponentType<{ readonly param: string | null }>;
}

const TABS: Readonly<Record<StandardsTab, Tab>> = {
  vocabularies: { labelKey: 'page.vocabularies', element: VocabulariesPage },
  properties: { labelKey: 'page.properties', element: PropertiesPage },
  classes: { labelKey: 'detail.classes', element: ResourceClassesPage },
};

/** A tab is the page on its list, and the current section on a member of it. */
const currentTab = (
  tab: StandardsTab,
  open: StandardsTab,
  param: string | null,
): 'page' | 'true' | undefined => {
  if (tab !== open) {
    return undefined;
  }
  return param === null ? 'page' : 'true';
};

export const StandardsPage = ({
  rest = [],
}: {
  readonly rest?: readonly string[];
}): ReactElement => {
  const { t } = useTranslator();
  const text = useStandardsText();
  const [segment = 'vocabularies', param = null] = rest;
  if (!isStandardsTab(segment)) {
    return <p role="alert">{t('shell.noPage')}</p>;
  }
  const Page = TABS[segment].element;
  return (
    <>
      {param === null ? <p className="lede">{text('standards.lede')}</p> : null}
      <nav className="tabs" aria-label={t('page.standards')}>
        <ul>
          {STANDARDS_TABS.map((tab) => (
            <li key={tab}>
              <a href={standardsHref(tab)} aria-current={currentTab(tab, segment, param)}>
                {t(TABS[tab].labelKey)}
              </a>
            </li>
          ))}
        </ul>
      </nav>
      <Page key={segment} param={param} />
    </>
  );
};
