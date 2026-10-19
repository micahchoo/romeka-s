/**
 * `#/resource-templates` is "Item types": the list, `.../new` asks what kind
 * of thing the type describes, `.../<id>` opens the editor. The route keeps
 * its upstream name so every old link still works.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { templateIdFromParam } from '../../api/parse';
import type { ResourceTemplate, TemplateId } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { NewType } from './ask';
import { TypeEditor } from './editor';
import './item-types.css';
import { TypeList } from './list';
import { useItemTypesText } from './messages';
import { typeDraftFrom } from './model';
import { loadTypeOptions, type TypeOptions } from './options';

const Loading = ({
  children,
}: {
  readonly children: (options: TypeOptions) => ReactElement;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(loadTypeOptions, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return children(state.data);
};

const loadType = async (
  id: TemplateId,
): Promise<{ readonly template: ResourceTemplate; readonly options: TypeOptions }> => {
  const [template, options] = await Promise.all([api.getResourceTemplate(id), loadTypeOptions()]);
  return { template, options };
};

const EditType = ({ templateId }: { readonly templateId: TemplateId }): ReactElement => {
  const { t } = useTranslator();
  const typesText = useItemTypesText();
  const state = useAsync(() => loadType(templateId), [templateId]);
  const [saved, setSaved] = useState<ResourceTemplate | null>(null);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.resourceTemplate')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  const template = saved ?? state.data.template;
  return (
    <>
      <TypeEditor
        template={template}
        initial={typeDraftFrom(state.data.template)}
        options={state.data.options}
        onSaved={setSaved}
      />
      <DeleteControl
        label={typesText('type.delete')}
        rowName={template.label}
        onConfirm={async () => {
          await api.deleteResourceTemplate(template.id);
          window.location.hash = '#/resource-templates';
        }}
      />
    </>
  );
};

export const ItemTypesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => (
  <ItemTypesRoute param={param} />
);

const ItemTypesRoute = ({ param }: { readonly param: string | null }): ReactElement => {
  const { t } = useTranslator();
  if (param === null) {
    return <TypeList />;
  }
  if (param === 'new') {
    return <Loading>{(options) => <NewType options={options} />}</Loading>;
  }
  const templateId = templateIdFromParam(param);
  if (templateId === null) {
    return <p role="alert">{t('route.invalidTemplateId', { param })}</p>;
  }
  return <EditType key={String(templateId)} templateId={templateId} />;
};
