import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Property, ResourceClass, Vocabulary, VocabularyId } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { VocabularyEditForm, isPermanentVocabulary } from './form';

/**
 * The API's `vocabulary_id` filter narrows the properties and classes to this
 * vocabulary. One page carries them all, so the detail shows every member.
 */
const MEMBERS_PER_PAGE = Number.MAX_SAFE_INTEGER;

interface Loaded {
  readonly vocabulary: Vocabulary;
  readonly properties: readonly Property[];
  readonly classes: readonly ResourceClass[];
}

const load = async (vocabularyId: VocabularyId): Promise<Loaded> => {
  const [vocabulary, properties, classes] = await Promise.all([
    api.getVocabulary(vocabularyId),
    api.listProperties({
      vocabulary_id: vocabularyId,
      perPage: MEMBERS_PER_PAGE,
      sortBy: 'id',
      sortOrder: 'asc',
    }),
    api.listResourceClasses({
      vocabulary_id: vocabularyId,
      perPage: MEMBERS_PER_PAGE,
      sortBy: 'id',
      sortOrder: 'asc',
    }),
  ]);
  return { vocabulary, properties: properties.items, classes: classes.items };
};

/**
 * `Admin/VocabularyController::showDetailsAction` with `::propertiesAction` and
 * `::classesAction`: one vocabulary, its own members, a link to every one, the
 * edit form and the delete control. A permanent vocabulary offers neither.
 */
export const VocabularyDetail = ({
  vocabularyId,
}: {
  readonly vocabularyId: VocabularyId;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(vocabularyId), [vocabularyId]);
  const [edited, setEdited] = useState<Vocabulary | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.vocabulary')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const { properties, classes } = state.data;
  const vocabulary = edited ?? state.data.vocabulary;
  return (
    <section>
      <h2>{vocabulary.label}</h2>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{vocabulary.id}</dd>
        <dt>{t('column.prefix')}</dt>
        <dd>{vocabulary.prefix}</dd>
        <dt>{t('detail.namespaceUri')}</dt>
        <dd>{vocabulary.namespaceUri}</dd>
        <dt>{t('column.comment')}</dt>
        <dd>{vocabulary.comment ?? t('common.none')}</dd>
        <dt>{t('column.owner')}</dt>
        <dd>
          {vocabulary.owner === null ? (
            t('common.noOwner')
          ) : (
            <a href={`#/users/${String(vocabulary.owner.id)}`}>
              {`#${String(vocabulary.owner.id)}`}
            </a>
          )}
        </dd>
      </dl>

      <h3>{t('column.properties')}</h3>
      {properties.length === 0 ? (
        <p>{t('detail.noProperties')}</p>
      ) : (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>{t('column.label')}</th>
                <th>{t('column.term')}</th>
              </tr>
            </thead>
            <tbody>
              {properties.map((property) => (
                <tr key={String(property.id)}>
                  <td>
                    <a href={`#/properties/${String(property.id)}`}>
                      {property.label ?? property.term}
                    </a>
                  </td>
                  <td>{property.term}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <h3>{t('detail.classes')}</h3>
      {classes.length === 0 ? (
        <p>{t('detail.noClasses')}</p>
      ) : (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>{t('column.label')}</th>
                <th>{t('column.term')}</th>
              </tr>
            </thead>
            <tbody>
              {classes.map((resourceClass) => (
                <tr key={String(resourceClass.id)}>
                  <td>
                    <a href={`#/resource-classes/${String(resourceClass.id)}`}>
                      {resourceClass.label ?? resourceClass.term}
                    </a>
                  </td>
                  <td>{resourceClass.term}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {isPermanentVocabulary(vocabulary) ? (
        <p>{t('vocabularies.permanent')}</p>
      ) : (
        <>
          <EditPanel label={t('vocabularies.editLabel')}>
            <VocabularyEditForm vocabulary={vocabulary} onSaved={setEdited} />
          </EditPanel>

          <DeleteControl
            label={t('vocabularies.deleteLabel')}
            rowName={vocabulary.label}
            onConfirm={async () => {
              await api.deleteVocabulary(vocabulary.id);
              window.location.hash = '#/vocabularies';
            }}
          />
        </>
      )}
    </section>
  );
};
