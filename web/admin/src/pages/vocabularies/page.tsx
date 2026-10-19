import type { ReactElement } from 'react';
import { Suspense, lazy } from 'react';
import { vocabularyIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { VocabularyCreatePage } from './form';
import { VocabularyDetail } from './detail';
import { VocabularyList } from './list';
import { common } from '../../i18n/namespaces/common';
import { standards } from '../../i18n/namespaces/standards';

/** The import form is its own chunk: most visits to the tab never open it. */
const VocabularyImportPage = lazy(() =>
  import('./import').then((module) => ({ default: module.VocabularyImportPage })),
);

/** The Vocabularies tab: its list, `new` to create one, `import` to import one, or an id. */
export const VocabulariesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator(common, standards);
  if (param === null) {
    return <VocabularyList />;
  }
  if (param === 'new') {
    return <VocabularyCreatePage />;
  }
  if (param === 'import') {
    return (
      <Suspense fallback={<p role="status">{t('loading.form')}</p>}>
        <VocabularyImportPage />
      </Suspense>
    );
  }
  const vocabularyId = vocabularyIdFromParam(param);
  if (vocabularyId === null) {
    return <p role="alert">{t('route.invalidVocabularyId', { param })}</p>;
  }
  return <VocabularyDetail key={String(vocabularyId)} vocabularyId={vocabularyId} />;
};
