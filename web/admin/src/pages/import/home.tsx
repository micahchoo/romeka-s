/**
 * `#/import`: where the files are, then the past imports.
 */
import type { ReactElement } from 'react';
import { useImportText } from './messages';
import { PastImports } from './past-imports';
import { SourcePicker } from './source-picker';

export const ImportHome = (): ReactElement => {
  const { t } = useImportText();
  return (
    <>
      <p className="lede">{t('home.lede')}</p>
      <SourcePicker />
      <PastImports />
    </>
  );
};
