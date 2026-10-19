import type { ReactElement } from 'react';
import { vocabularyIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { VocabularyCreatePage } from './form';
import { VocabularyDetail } from './detail';
import { VocabularyList } from './list';

/** The Vocabularies tab: its list, `new` to create one, or a vocabulary id. */
export const VocabulariesPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator();
  if (param === null) {
    return <VocabularyList />;
  }
  if (param === 'new') {
    return <VocabularyCreatePage />;
  }
  const vocabularyId = vocabularyIdFromParam(param);
  if (vocabularyId === null) {
    return <p role="alert">{t('route.invalidVocabularyId', { param })}</p>;
  }
  return <VocabularyDetail key={String(vocabularyId)} vocabularyId={vocabularyId} />;
};
