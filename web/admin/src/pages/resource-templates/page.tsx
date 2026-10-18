import type { ReactElement } from 'react';
import { templateIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { ResourceTemplateCreatePage } from './form';
import { ResourceTemplateDetail } from './detail';
import { ResourceTemplateList } from './list';

/** `#/resource-templates` lists, `.../new` creates, `.../<id>` edits. */
export const ResourceTemplatesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator();
  if (param === null) {
    return <ResourceTemplateList />;
  }
  if (param === 'new') {
    return <ResourceTemplateCreatePage />;
  }
  const templateId = templateIdFromParam(param);
  if (templateId === null) {
    return <p role="alert">{t('route.invalidTemplateId', { param })}</p>;
  }
  return <ResourceTemplateDetail key={String(templateId)} templateId={templateId} />;
};
