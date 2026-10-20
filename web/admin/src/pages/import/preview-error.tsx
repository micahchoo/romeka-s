/**
 * The preview's refusal, in the reader's language: one message per
 * `error.key` the server answers, with its detail when it sent one. Every
 * step that previews shows it.
 */
import type { ReactElement } from 'react';
import type { Preview, PreviewErrorKey } from '../../api/import-preview';
import { useImportText } from './messages';
import type { ImportMessageKey } from './messages';

export const PREVIEW_ERROR_TEXT: Readonly<Record<PreviewErrorKey, ImportMessageKey>> = {
  patternMissing: 'error.patternMissing',
  patternSyntax: 'error.patternSyntax',
  patternIdentifier: 'error.patternIdentifier',
  csvParse: 'error.csvParse',
  delimiter: 'error.delimiter',
  headerRow: 'error.headerRow',
  joinColumn: 'error.joinColumn',
  csvUnreadable: 'error.csvUnreadable',
  templateMissing: 'error.templateMissing',
  termUnknown: 'error.termUnknown',
  typeExists: 'error.typeExists',
};

export const PreviewError = ({ preview }: { readonly preview: Preview | null }): ReactElement | null => {
  const { t } = useImportText();
  const error = preview?.error;
  if (error === undefined) {
    return null;
  }
  return (
    <p role="alert">
      {t(PREVIEW_ERROR_TEXT[error.key])}
      {error.detail === undefined ? null : (
        <>
          {' '}
          <span className="import-mono">{error.detail}</span>
        </>
      )}
    </p>
  );
};
