import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { ListPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 25;

/** `Admin/VocabularyController::browseAction`: label, prefix, namespace. */
export const VocabularyList = (): ReactElement => {
  const { t } = useTranslator();
  const resource = t('resource.vocabularies');
  return (
    <div>
      <p>
        <a href="#/vocabularies/new">{t('vocabularies.add')}</a>
      </p>
      <ListPage
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={(page) =>
          api.listVocabularies({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' })
        }
        rowKey={(vocabulary) => String(vocabulary.id)}
        columns={[
          {
            header: t('column.label'),
            cell: (vocabulary) => (
              <a href={`#/vocabularies/${String(vocabulary.id)}`}>{vocabulary.label}</a>
            ),
          },
          { header: t('column.prefix'), cell: (vocabulary) => vocabulary.prefix },
          { header: t('column.namespace'), cell: (vocabulary) => vocabulary.namespaceUri },
          { header: t('column.comment'), cell: (vocabulary) => vocabulary.comment ?? '' },
        ]}
      />
    </div>
  );
};
