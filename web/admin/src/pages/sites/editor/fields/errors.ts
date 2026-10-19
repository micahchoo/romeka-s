/**
 * `validate()` issues as messages for a control. The draft model owns the rule,
 * the catalogue owns the wording, and this module is the one place they meet.
 */
import type { KeyOf, MessageParams } from '../../../../i18n';
import type { editor } from '../../../../i18n/namespaces/editor';
import type { ValidationIssue } from '../draft';

export interface FieldMessage {
  readonly key: KeyOf<typeof editor>;
  readonly params?: MessageParams;
}

/** The message one issue renders, with the bound it names when it has one. */
export const issueMessage = (issue: ValidationIssue): FieldMessage => {
  switch (issue.kind) {
    case 'required':
      return { key: 'field.errorRequired' };
    case 'type':
      return { key: 'field.errorInteger' };
    case 'min':
      return { key: 'field.errorMin', params: { min: issue.min } };
    case 'max':
      return { key: 'field.errorMax', params: { max: issue.max } };
  }
};

/**
 * The first issue on `path` for one block. A repeat entry field is named
 * `key.index.subkey`, so an exact match is the only one a control owns.
 */
export const issueFor = (
  issues: readonly ValidationIssue[],
  blockKey: ValidationIssue['blockKey'],
  path: string,
): ValidationIssue | null =>
  issues.find((issue) => issue.blockKey === blockKey && issue.field === path) ?? null;
