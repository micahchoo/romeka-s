import type { ReactElement } from 'react';
import { Suspense, lazy, useState } from 'react';
import { api } from '../../api/client';
import type { Property, ResourceClass, Vocabulary, VocabularyId } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { DefinedTerm } from '../standards/definition';
import { standardsHref } from '../standards/routes';
import { VocabularyEditForm, isPermanentVocabulary } from './form';
import {
  PROPERTY_KIND,
  RESOURCE_CLASS_KIND,
  VocabularyMemberAddForm,
} from './member';
import type { MemberKind, VocabularyMemberRow } from './member';
import { common } from '../../i18n/namespaces/common';
import { standards } from '../../i18n/namespaces/standards';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof standards>;

/** The update panel is its own chunk, fetched when a vocabulary page opens. */
const VocabularyUpdate = lazy(() =>
  import('./update').then((module) => ({ default: module.VocabularyUpdate })),
);

/**
 * The API's `vocabulary_id` filter narrows the properties and classes to this
 * vocabulary. One page carries them all, so the detail shows every member.
 */
const MEMBERS_PER_PAGE = Number.MAX_SAFE_INTEGER;

interface Members {
  readonly properties: readonly Property[];
  readonly classes: readonly ResourceClass[];
}

interface Loaded extends Members {
  readonly vocabulary: Vocabulary;
}

/**
 * One kind of member: a table of them, and on a vocabulary that may change,
 * the form that adds one to it in place.
 */
const MemberTable = <MemberT extends VocabularyMemberRow>({
  kind,
  members,
  emptyKey,
  vocabulary,
  onAdded,
}: {
  readonly kind: MemberKind<MemberT>;
  readonly members: readonly MemberT[];
  readonly emptyKey: MessageId;
  /** The vocabulary the add form writes to, or `null` for a permanent one. */
  readonly vocabulary: VocabularyId | null;
  readonly onAdded: (member: MemberT) => void;
}): ReactElement => {
  const { t } = useTranslator(common, standards);
  return (
    <>
      {members.length === 0 ? (
        <p>{t(emptyKey)}</p>
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
              {members.map((member) => (
                <tr key={String(member.id)}>
                  <td>
                    <a href={standardsHref(kind.tab, member.id)}>{member.label ?? member.term}</a>
                  </td>
                  <td>
                    <DefinedTerm term={member.term} comment={member.comment}>
                      {member.term}
                    </DefinedTerm>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {vocabulary === null ? null : (
        <EditPanel label={t(kind.addKey)}>
          <VocabularyMemberAddForm kind={kind} vocabulary={vocabulary} onAdded={onAdded} />
        </EditPanel>
      )}
    </>
  );
};

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
 * edit form and the delete control. A vocabulary that is not permanent also
 * takes new properties and classes here, so one page builds a whole vocabulary.
 */
export const VocabularyDetail = ({
  vocabularyId,
}: {
  readonly vocabularyId: VocabularyId;
}): ReactElement => {
  const { t } = useTranslator(common, standards);
  const state = useAsync(() => load(vocabularyId), [vocabularyId]);
  const [edited, setEdited] = useState<Vocabulary | null>(null);
  const [added, setAdded] = useState<Members>({ properties: [], classes: [] });
  const [reloaded, setReloaded] = useState<Members | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.vocabulary')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const vocabulary = edited ?? state.data.vocabulary;
  const editable = !isPermanentVocabulary(vocabulary);
  const stored = reloaded ?? state.data;
  const properties = [...stored.properties, ...added.properties];
  const classes = [...stored.classes, ...added.classes];
  const reload = (): void => {
    void load(vocabularyId).then((fresh) => {
      setAdded({ properties: [], classes: [] });
      setReloaded(fresh);
    });
  };
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
      <MemberTable
        kind={PROPERTY_KIND}
        members={properties}
        emptyKey="detail.noProperties"
        vocabulary={editable ? vocabulary.id : null}
        onAdded={(property) => {
          setAdded((prior) => ({ ...prior, properties: [...prior.properties, property] }));
        }}
      />

      <h3>{t('detail.classes')}</h3>
      <MemberTable
        kind={RESOURCE_CLASS_KIND}
        members={classes}
        emptyKey="detail.noClasses"
        vocabulary={editable ? vocabulary.id : null}
        onAdded={(resourceClass) => {
          setAdded((prior) => ({ ...prior, classes: [...prior.classes, resourceClass] }));
        }}
      />

      {editable ? (
        <>
          <EditPanel label={t('vocabularies.editLabel')}>
            <VocabularyEditForm vocabulary={vocabulary} onSaved={setEdited} />
          </EditPanel>

          <Suspense fallback={null}>
            <VocabularyUpdate vocabulary={vocabulary} onApplied={reload} />
          </Suspense>

          <DeleteControl
            label={t('vocabularies.deleteLabel')}
            rowName={vocabulary.label}
            onConfirm={async () => {
              await api.deleteVocabulary(vocabulary.id);
              window.location.hash = standardsHref('vocabularies');
            }}
          />
        </>
      ) : (
        <p>{t('vocabularies.permanent')}</p>
      )}
    </section>
  );
};
