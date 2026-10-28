/**
 * How many of a type's items fill one field, and the link to the ones that do not. Both numbers
 * are the server's (`/api/romeka/field-counts`) for the reader, so the link opens a list whose
 * total is the number it states.
 */
import type { ReactElement } from 'react';
import type { FieldCounts } from '../../api/places';
import type { PropertyId, TemplateId } from '../../api/types';
import { useTranslator } from '../../i18n';
import { itemTypes } from '../../i18n/namespaces/item-types';
import { fieldPlaceHref } from '../items/browse';

export interface FieldFillProps {
  readonly template: TemplateId;
  readonly property: PropertyId;
  readonly counts: FieldCounts;
}

export const FieldFill = ({ template, property, counts }: FieldFillProps): ReactElement | null => {
  const { t, number } = useTranslator(itemTypes);
  const { total } = counts;
  if (total === 0) {
    return null;
  }
  const filled = counts.filled.get(property) ?? 0;
  const empty = total - filled;
  return (
    <p className="type-field-fill">
      {total === 1
        ? t('fill.ofOne', { filled: number(filled) })
        : t('fill.ofMany', { filled: number(filled), total: number(total) })}
      {empty === 0 ? null : (
        <>
          {' '}
          <a href={fieldPlaceHref(template, property)}>{t('fill.emptyLink', { count: number(empty) })}</a>
        </>
      )}
    </p>
  );
};
