/**
 * A page that names a field by its vocabulary label instead of `plainName`.
 * ESLint must refuse it (`no-restricted-syntax`, the field name rule);
 * `tests/field-names.test.ts` is the check. The default `eslint .` run ignores
 * this directory, so the fixture can only fail when a test asks for that failure.
 */
import type { TermLabels } from '../../src/i18n';

export const nameOf = (property: { readonly term: string; readonly label: string | null }, text: TermLabels): string | null =>
  text.label(property.term, property.label);

export const nameOfTerm = (term: string, text: TermLabels): string | null => text.label(term, null);
