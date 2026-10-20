/**
 * The control of the `propertyList` kind: an ordered list of property terms,
 * such as the fields an item-with-metadata block shows. Only that block uses
 * it, so its field loads as its own chunk (PERF.md, Decision 25).
 */
import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { useEditorText } from '../messages';
import { asList } from './controlBase';
import type { ControlProps, FieldOf } from './controlBase';

const PropertyListField = lazy(() =>
  import('./PropertyListField').then((module) => ({ default: module.PropertyListField })),
);

const Loading = (): ReactElement => {
  const { t } = useTranslator(common);
  return (
    <p className="field-note" role="status">
      {t('loading.form')}
    </p>
  );
};

export const PropertyListControl = ({
  field,
  id,
  value,
  onChange,
}: ControlProps & { readonly field: FieldOf<'propertyList'> }): ReactElement => {
  const e = useEditorText();
  return (
    <Suspense fallback={<Loading />}>
      <PropertyListField
        id={id}
        label={e(field.label)}
        help={e(field.help)}
        value={asList(value)}
        onChange={onChange}
      />
    </Suspense>
  );
};
