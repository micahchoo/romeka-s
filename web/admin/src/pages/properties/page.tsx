import type { ReactElement } from 'react';
import { propertyIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import {
  PROPERTY_KIND,
  VocabularyMemberCreatePage,
} from '../vocabularies/member';
import { PropertyDetail } from './detail';
import { PropertyList } from './list';
import { standards } from '../../i18n/namespaces/standards';

/** The Properties tab: its list, `new` to create one, or a property id. */
export const PropertiesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator(standards);
  if (param === null) {
    return <PropertyList />;
  }
  if (param === 'new') {
    return <VocabularyMemberCreatePage kind={PROPERTY_KIND} />;
  }
  const propertyId = propertyIdFromParam(param);
  if (propertyId === null) {
    return <p role="alert">{t('route.invalidPropertyId', { param })}</p>;
  }
  return <PropertyDetail key={String(propertyId)} propertyId={propertyId} />;
};
