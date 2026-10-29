/**
 * Advanced > Standards: the vocabularies, properties and classes that fields
 * and item types are saved as, one tab each. Each tab is a route
 * (`#/standards/<tab>/<id>`), and upstream's own routes for the three reach the
 * same tabs through the router.
 */
import type { ComponentType, ReactElement } from 'react';
import type { KeyOf, MessageKey } from '../../i18n';
import { useLesson } from '../../app/lessons/lesson';
import { useTranslator } from '../../i18n';
import { useStandardsText } from './messages';
import { PropertiesPage } from '../properties/page';
import { ResourceClassesPage } from '../resource-classes/page';
import { VocabulariesPage } from '../vocabularies/page';
import { isStandardsTab, STANDARDS_TABS, standardsHref } from './routes';
import type { StandardsTab } from './routes';
import { common } from '../../i18n/namespaces/common';
import { standards } from '../../i18n/namespaces/standards';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof standards>;

interface Tab {
  readonly labelKey: MessageId;
  readonly element: ComponentType<{ readonly param: string | null }>;
}

const TABS: Readonly<Record<StandardsTab, Tab>> = {
  vocabularies: { labelKey: 'page.standards', element: VocabulariesPage },
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
  const { t } = useTranslator(common, standards);
  const text = useStandardsText();
  const [segment = 'vocabularies', param = null] = rest;
  const lesson = useLesson('standards', param === null);
  if (!isStandardsTab(segment)) {
    return <p role="alert">{t('shell.noPage')}</p>;
  }
  const Page = TABS[segment].element;
  return (
    <>
      {param === null ? (
        <p className="lede">
          {text('standards.lede')} {lesson.toggle}
        </p>
      ) : null}
      {lesson.panel}
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
