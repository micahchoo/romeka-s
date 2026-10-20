/**
 * The fields a tag or a spreadsheet column can map to: every property the
 * vocabularies hold, in the reader's language, plus the new local fields the
 * plan makes. A local field's term is the server's, read from the grid's
 * columns, never derived here.
 */
import { api } from '../../api/client';
import type { RowsPreview } from '../../api/import-preview';
import type { Property } from '../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../app/all-pages';
import type { TermLabels } from '../../i18n/termText';

export interface FieldOption {
  readonly term: string;
  readonly label: string;
}

/** The identifier comes from the file names, so nothing maps to it. */
const IDENTIFIER = 'dcterms:identifier';

export const loadProperties = (): Promise<readonly Property[]> =>
  allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' }));

export const fieldOptions = (
  properties: readonly Property[],
  columns: RowsPreview['columns'],
  labels: TermLabels,
): readonly FieldOption[] => {
  const known = properties.map((property) => ({
    term: property.term,
    label: labels.label(property.term, property.label) ?? property.term,
  }));
  const terms = new Set(known.map((option) => option.term));
  const local = columns
    .filter((column) => !terms.has(column.term))
    .map((column) => ({ term: column.term, label: column.label }));
  return [...known, ...local].filter((option) => option.term !== IDENTIFIER);
};
