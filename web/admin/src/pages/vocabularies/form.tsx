/**
 * `Admin/VocabularyController::editAction` and the create surface beside it.
 *
 * The edit posts `['isPartial' => true]` upstream, so the port sends
 * `PATCH /api/vocabularies/<id>`. A full `PUT` must carry the `o:class` and
 * `o:property` lists, and one that omits either removes every member of that
 * kind; each member owns its own form, so this form never sends them.
 * `VocabularyForm::init` adds `o:prefix` on create only, so the edit does not
 * offer it and the stored prefix stays.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { User, UserId, Vocabulary, VocabularyWrite } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { OptionSelect, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { userOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { standardsHref } from '../standards/routes';
import { common } from '../../i18n/namespaces/common';
import { standards } from '../../i18n/namespaces/standards';

/**
 * `Vocabulary::PREFIX_REGEX`, `/^[a-z0-9_-]+$/i`: the prefix a create accepts.
 */
const PREFIX_PATTERN = /^[a-z0-9_-]+$/i;

/**
 * `VocabularyRepresentation::isPermanent`: Dublin Core and Dublin Core Type are
 * part of the software. Upstream refuses to edit or delete them; the port's API
 * has no such check, so the admin surface keeps the rule.
 */
export const isPermanentVocabulary = (vocabulary: Vocabulary): boolean =>
  vocabulary.prefix === 'dcterms' || vocabulary.prefix === 'dctype';

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

interface VocabularyValues {
  readonly namespaceUri: string;
  readonly prefix: string;
  readonly label: string;
  readonly comment: string;
  readonly owner: UserId | null;
}

const valuesFrom = (vocabulary: Vocabulary | null): VocabularyValues => ({
  namespaceUri: vocabulary?.namespaceUri ?? '',
  prefix: vocabulary?.prefix ?? '',
  label: vocabulary?.label ?? '',
  comment: vocabulary?.comment ?? '',
  owner: vocabulary?.owner?.id ?? null,
});

const VocabularyEditor = ({
  vocabulary,
  users,
  onSaved,
}: {
  readonly vocabulary: Vocabulary | null;
  readonly users: readonly User[];
  readonly onSaved: (vocabulary: Vocabulary) => void;
}): ReactElement => {
  const { t } = useTranslator(common, standards);
  const create = vocabulary === null;
  const stored = valuesFrom(vocabulary);
  const [namespaceUri, setNamespaceUri] = useState(stored.namespaceUri);
  const [prefix, setPrefix] = useState(stored.prefix);
  const [label, setLabel] = useState(stored.label);
  const [comment, setComment] = useState(stored.comment);
  const [owner, setOwner] = useState<UserId | null>(stored.owner);
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (label.trim() === '') {
      setFormError(t('form.errorEmptyLabel'));
      return;
    }
    if (namespaceUri.trim() === '') {
      setFormError(t('vocabularies.errorEmptyNamespace'));
      return;
    }
    if (create) {
      if (prefix.trim() === '') {
        setFormError(t('vocabularies.errorEmptyPrefix'));
        return;
      }
      if (!PREFIX_PATTERN.test(prefix)) {
        setFormError(t('vocabularies.errorPrefixPattern'));
        return;
      }
    }
    setFormError(null);
    const write: VocabularyWrite = {
      namespaceUri,
      prefix,
      label,
      comment: nullable(comment),
      owner,
    };
    save(async () => {
      if (vocabulary === null) {
        const created = await api.createVocabulary(write);
        window.location.hash = standardsHref('vocabularies', created.id);
      } else {
        onSaved(await api.updateVocabulary(vocabulary.id, write));
      }
    });
  };

  return (
    <form onSubmit={submit}>
      <fieldset>
        <legend>{t('vocabularies.legend')}</legend>
        <TextField
          id="vocabulary-namespace-uri"
          label={t('detail.namespaceUri')}
          value={namespaceUri}
          onChange={setNamespaceUri}
        />
        {create ? (
          <TextField
            id="vocabulary-prefix"
            label={t('column.prefix')}
            value={prefix}
            onChange={setPrefix}
          />
        ) : null}
        <TextField
          id="vocabulary-label"
          label={t('column.label')}
          value={label}
          onChange={setLabel}
        />
        <p>
          <label htmlFor="vocabulary-comment">{t('column.comment')}</label>
          <textarea
            id="vocabulary-comment"
            rows={2}
            value={comment}
            onChange={(event) => {
              setComment(event.target.value);
            }}
          />
        </p>
        <OptionSelect
          id="vocabulary-owner"
          label={t('field.owner')}
          options={userOptions(users)}
          value={owner}
          onChange={setOwner}
          emptyLabel={t('common.noOwner')}
        />
      </fieldset>
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow
        label={create ? t('vocabularies.add') : t('action.save')}
        busy={saveState.status === 'saving'}
      />
    </form>
  );
};

const loadUsers = async (): Promise<readonly User[]> =>
  (await api.listUsers({ perPage: 200, sortBy: 'name' })).items;

/** The edit form on the vocabulary detail view. */
export const VocabularyEditForm = ({
  vocabulary,
  onSaved,
}: {
  readonly vocabulary: Vocabulary;
  readonly onSaved: (vocabulary: Vocabulary) => void;
}): ReactElement => {
  const { t } = useTranslator(common, standards);
  const state = useAsync(loadUsers, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <VocabularyEditor vocabulary={vocabulary} users={state.data} onSaved={onSaved} />;
};

/**
 * `#/standards/vocabularies/new`. A create opens the new vocabulary's page,
 * where its properties and classes are added.
 */
export const VocabularyCreatePage = (): ReactElement => {
  const { t } = useTranslator(common, standards);
  const state = useAsync(loadUsers, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.vocabularyForm')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <VocabularyEditor vocabulary={null} users={state.data} onSaved={() => undefined} />;
};
