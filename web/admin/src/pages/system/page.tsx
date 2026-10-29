/**
 * Advanced > System: the facts about this installation and the tools that
 * check it, one tab each (`#/system/<tab>`). Upstream's System information,
 * Modules and Search query pages are its tabs, and their routes reach the same
 * tabs through the router. Upstream's Browse columns page is not here: its
 * choice was never saved, so the page offered a change that did nothing.
 */
import type { ComponentType, ReactElement } from 'react';
import type { KeyOf, MessageKey } from '../../i18n';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';
import { SystemInfoPage } from './about';
import { ModulesPage } from './modules';
import { QueryPage } from './query';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof admin>;

type SystemTab = 'about' | 'modules' | 'query';

const TABS: Readonly<Record<SystemTab, { readonly labelKey: MessageId; readonly element: ComponentType }>> = {
  about: { labelKey: 'system.tabAbout', element: SystemInfoPage },
  modules: { labelKey: 'system.tabModules', element: ModulesPage },
  query: { labelKey: 'system.tabQuery', element: QueryPage },
};

const SYSTEM_TABS: readonly SystemTab[] = ['about', 'modules', 'query'];

const isSystemTab = (value: string): value is SystemTab => SYSTEM_TABS.some((tab) => tab === value);

const systemHref = (tab: SystemTab): string => (tab === 'about' ? '#/system' : `#/system/${tab}`);

export const SystemPage = ({ rest = [] }: { readonly rest?: readonly string[] }): ReactElement => {
  const { t } = useTranslator(common, admin);
  const [segment = 'about'] = rest;
  if (!isSystemTab(segment)) {
    return <p role="alert">{t('shell.noPage')}</p>;
  }
  const Tab = TABS[segment].element;
  return (
    <>
      <nav className="tabs" aria-label={t('page.system')}>
        <ul>
          {SYSTEM_TABS.map((tab) => (
            <li key={tab}>
              <a href={systemHref(tab)} aria-current={tab === segment ? 'page' : undefined}>
                {t(TABS[tab].labelKey)}
              </a>
            </li>
          ))}
        </ul>
      </nav>
      <Tab key={segment} />
    </>
  );
};
