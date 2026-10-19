import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Property, PropertyId, Vocabulary } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { PROPERTY_KIND, MemberDetail } from '../vocabularies/member';
import { standards } from '../../i18n/namespaces/standards';

interface Loaded {
  readonly property: Property;
  readonly vocabulary: Vocabulary;
}

/** The property names its vocabulary by id, so the label takes a second read. */
const load = async (propertyId: PropertyId): Promise<Loaded> => {
  const property = await api.getProperty(propertyId);
  const vocabulary = await api.getVocabulary(property.vocabulary.id);
  return { property, vocabulary };
};

/**
 * `Admin/PropertyController::showDetailsAction`: every member of one property,
 * with the edit and delete controls the API's write surface offers.
 */
export const PropertyDetail = ({
  propertyId,
}: {
  readonly propertyId: PropertyId;
}): ReactElement => {
  const { t } = useTranslator(standards);
  const state = useAsync(() => load(propertyId), [propertyId]);
  const [edited, setEdited] = useState<Property | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.property')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const { property, vocabulary } = state.data;
  return (
    <MemberDetail
      kind={PROPERTY_KIND}
      member={edited ?? property}
      vocabulary={vocabulary}
      onSaved={setEdited}
    />
  );
};
