import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ClassId, ResourceClass, Vocabulary } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { RESOURCE_CLASS_KIND, MemberDetail } from '../vocabularies/member';
import { standards } from '../../i18n/namespaces/standards';

interface Loaded {
  readonly resourceClass: ResourceClass;
  readonly vocabulary: Vocabulary;
}

/** The class names its vocabulary by id, so the label takes a second read. */
const load = async (classId: ClassId): Promise<Loaded> => {
  const resourceClass = await api.getResourceClass(classId);
  const vocabulary = await api.getVocabulary(resourceClass.vocabulary.id);
  return { resourceClass, vocabulary };
};

/**
 * `Admin/ResourceClassController::showDetailsAction`: every member of one
 * resource class, with the edit and delete controls the API's write surface
 * offers.
 */
export const ResourceClassDetail = ({ classId }: { readonly classId: ClassId }): ReactElement => {
  const { t } = useTranslator(standards);
  const state = useAsync(() => load(classId), [classId]);
  const [edited, setEdited] = useState<ResourceClass | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.resourceClass')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const { resourceClass, vocabulary } = state.data;
  return (
    <MemberDetail
      kind={RESOURCE_CLASS_KIND}
      member={edited ?? resourceClass}
      vocabulary={vocabulary}
      onSaved={setEdited}
    />
  );
};
