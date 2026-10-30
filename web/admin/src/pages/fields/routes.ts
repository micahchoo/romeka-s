/** The values board of one field: `#/fields/<property id>`. */
import type { PropertyId } from '../../api/types';

export const valuesHref = (property: PropertyId | number): string => `#/fields/${String(property)}`;
