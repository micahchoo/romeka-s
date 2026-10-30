/** The values board of one field: `#/values/<property id>` (`#/fields/<id>`, its first address, still opens it). */
import type { PropertyId } from '../../api/types';

export const valuesHref = (property: PropertyId | number): string => `#/values/${String(property)}`;
