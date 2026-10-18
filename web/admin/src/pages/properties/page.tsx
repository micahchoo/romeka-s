import type { ReactElement } from 'react';
import { propertyIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import {
  PROPERTY_KIND,
  VocabularyMemberCreatePage,
} from '../vocabularies/member';
import { PropertyDetail } from './detail';
import { PropertyList } from './list';

/** `#/properties` lists, `#/properties/new` creates, `#/properties/<id>` edits. */
export const PropertiesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator();
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
