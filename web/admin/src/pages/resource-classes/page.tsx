import type { ReactElement } from 'react';
import { classIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import {
  RESOURCE_CLASS_KIND,
  VocabularyMemberCreatePage,
} from '../vocabularies/member';
import { ResourceClassDetail } from './detail';
import { ResourceClassList } from './list';
import { standards } from '../../i18n/namespaces/standards';

/** The Classes tab: its list, `new` to create one, or a class id. */
export const ResourceClassesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator(standards);
  if (param === null) {
    return <ResourceClassList />;
  }
  if (param === 'new') {
    return <VocabularyMemberCreatePage kind={RESOURCE_CLASS_KIND} />;
  }
  const classId = classIdFromParam(param);
  if (classId === null) {
    return <p role="alert">{t('route.invalidClassId', { param })}</p>;
  }
  return <ResourceClassDetail key={String(classId)} classId={classId} />;
};
