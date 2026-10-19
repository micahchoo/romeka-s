import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, Vocabulary } from '../../api/types';
import { ListPage } from '../../app/list';
import { useTranslator } from '../../i18n';
import { DefinedTerm } from '../standards/definition';
import { standardsHref } from '../standards/routes';

const PER_PAGE = 25;

interface VocabularyRow {
  readonly vocabulary: Vocabulary;
  readonly properties: number;
  readonly classes: number;
}

/**
 * The member counts come from each collection's total under the
 * `vocabulary_id` filter, two small reads per vocabulary on the page.
 */
const load = async (page: number): Promise<ApiPage<VocabularyRow>> => {
  const vocabularies = await api.listVocabularies({
    page,
    perPage: PER_PAGE,
    sortBy: 'id',
    sortOrder: 'asc',
  });
  const items = await Promise.all(
    vocabularies.items.map(async (vocabulary) => {
      const filter = { vocabulary_id: vocabulary.id, perPage: 1 };
      const [properties, classes] = await Promise.all([
        api.listProperties(filter),
        api.listResourceClasses(filter),
      ]);
      return { vocabulary, properties: properties.total, classes: classes.total };
    }),
  );
  return { ...vocabularies, items };
};

/** `Admin/VocabularyController::browseAction`: label, prefix, namespace, members. */
export const VocabularyList = (): ReactElement => {
  const { t, number } = useTranslator();
  const resource = t('resource.vocabularies');
  return (
    <div>
      <p>
        <a href={standardsHref('vocabularies', 'new')}>{t('vocabularies.add')}</a>
      </p>
      <ListPage<VocabularyRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.vocabulary.id)}
        columns={[
          {
            header: t('column.label'),
            cell: ({ vocabulary }) => (
              <DefinedTerm term={vocabulary.label} comment={vocabulary.comment}>
                <a href={standardsHref('vocabularies', vocabulary.id)}>{vocabulary.label}</a>
              </DefinedTerm>
            ),
          },
          { header: t('column.prefix'), cell: ({ vocabulary }) => vocabulary.prefix },
          { header: t('column.namespace'), cell: ({ vocabulary }) => vocabulary.namespaceUri },
          {
            header: t('column.properties'),
            cell: (row) => number(row.properties),
            numeric: true,
          },
          { header: t('detail.classes'), cell: (row) => number(row.classes), numeric: true },
        ]}
      />
    </div>
  );
};
