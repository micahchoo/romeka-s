import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { JsonValue, Module } from '../../api/types';
import { ListPage } from '../../app/list';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

const PER_PAGE = 25;

const isJsonObject = (value: JsonValue): value is { readonly [key: string]: JsonValue } =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/** A string member of the parsed `module.ini` object, or null. */
export const iniString = (ini: JsonValue | null, key: string): string | null => {
  if (ini === null || !isJsonObject(ini)) {
    return null;
  }
  const value = ini[key];
  return typeof value === 'string' ? value : null;
};

/** `Admin/ModuleController::browseAction`: name, version, state, author. */
export const ModulesPage = (): ReactElement => {
  const { t } = useTranslator(common, admin);
  const resource = t('resource.modules');
  return (
    <ListPage<Module>
      resourceLabel={resource}
      emptyMessage={t('list.empty', { resource })}
      load={(page) => api.listModules({ page, perPage: PER_PAGE })}
      rowKey={(module) => module.id}
      columns={[
        { header: t('column.id'), cell: (module) => module.id, numeric: true },
        { header: t('column.name'), cell: (module) => iniString(module.ini, 'name') ?? '' },
        { header: t('column.version'), cell: (module) => iniString(module.ini, 'version') ?? '' },
        { header: t('column.state'), cell: (module) => module.state },
        { header: t('column.author'), cell: (module) => iniString(module.ini, 'author') ?? '' },
        {
          header: t('column.description'),
          cell: (module) => iniString(module.ini, 'description') ?? '',
        },
      ]}
    />
  );
};
