/**
 * The write surface a property and a resource class share.
 *
 * `ResourceClassAdapter` and `PropertyAdapter` hydrate and validate the same
 * five members (`crates/omeka-api/src/write/vocab_member.rs`), so one editor
 * serves both and a [`MemberKind`] row names the endpoint, the route and the
 * message keys that differ.
 *
 * `o:owner` is write-only here. `AbstractVocabularyMemberRepresentation::getJsonLd`
 * carries no owner, so the form offers an owner select on create and omits the
 * member on update, which keeps the stored owner.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type {
  Property,
  ResourceClass,
  User,
  UserId,
  Vocabulary,
  VocabularyId,
  VocabularyMemberWrite,
} from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel, OptionSelect, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { userOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';

export type VocabularyMemberRow = Property | ResourceClass;

/** One member adapter: its resource name, its route and its endpoint calls. */
export interface MemberKind<MemberT extends VocabularyMemberRow> {
  readonly name: 'properties' | 'resource_classes';
  readonly route: 'properties' | 'resource-classes';
  readonly addKey: MessageKey;
  readonly editKey: MessageKey;
  readonly deleteKey: MessageKey;
  readonly loadingKey: MessageKey;
  readonly create: (write: VocabularyMemberWrite) => Promise<MemberT>;
  readonly update: (id: MemberT['id'], write: VocabularyMemberWrite) => Promise<MemberT>;
  readonly remove: (id: MemberT['id']) => Promise<MemberT>;
}

export const PROPERTY_KIND: MemberKind<Property> = {
  name: 'properties',
  route: 'properties',
  addKey: 'properties.add',
  editKey: 'properties.editLabel',
  deleteKey: 'properties.deleteLabel',
  loadingKey: 'loading.propertyForm',
  create: (write) => api.createProperty(write),
  update: (id, write) => api.updateProperty(id, write),
  remove: (id) => api.deleteProperty(id),
};

export const RESOURCE_CLASS_KIND: MemberKind<ResourceClass> = {
  name: 'resource_classes',
  route: 'resource-classes',
  addKey: 'resourceClasses.add',
  editKey: 'resourceClasses.editLabel',
  deleteKey: 'resourceClasses.deleteLabel',
  loadingKey: 'loading.resourceClassForm',
  create: (write) => api.createResourceClass(write),
  update: (id, write) => api.updateResourceClass(id, write),
  remove: (id) => api.deleteResourceClass(id),
};

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

interface MemberEditorProps<MemberT extends VocabularyMemberRow> {
  readonly kind: MemberKind<MemberT>;
  /** The stored member, or `null` to create one. */
  readonly member: MemberT | null;
  /** The vocabularies a create may name. */
  readonly vocabularies: readonly Vocabulary[];
  /** The users a create may name as owner. */
  readonly users: readonly User[];
  readonly onSaved: (member: MemberT) => void;
}

const MemberEditor = <MemberT extends VocabularyMemberRow>({
  kind,
  member,
  vocabularies,
  users,
  onSaved,
}: MemberEditorProps<MemberT>): ReactElement => {
  const { t } = useTranslator();
  const create = member === null;
  const [localName, setLocalName] = useState(member?.localName ?? '');
  const [label, setLabel] = useState(member?.label ?? '');
  const [comment, setComment] = useState(member?.comment ?? '');
  const [owner, setOwner] = useState<UserId | null>(null);
  const [vocabulary, setVocabulary] = useState<VocabularyId | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (localName.trim() === '') {
      setFormError(t('member.errorEmptyLocalName'));
      return;
    }
    if (label.trim() === '') {
      setFormError(t('form.errorEmptyLabel'));
      return;
    }
    let vocabularyId: VocabularyId;
    if (member === null) {
      if (vocabulary === null) {
        setFormError(t('member.errorEmptyVocabulary'));
        return;
      }
      vocabularyId = vocabulary;
    } else {
      vocabularyId = member.vocabulary.id;
    }
    setFormError(null);
    const write: VocabularyMemberWrite = {
      localName,
      label,
      comment: nullable(comment),
      owner,
      vocabulary: vocabularyId,
    };
    save(async () => {
      if (member === null) {
        const created = await kind.create(write);
        window.location.hash = `#/${kind.route}/${String(created.id)}`;
      } else {
        onSaved(await kind.update(member.id, write));
      }
    });
  };

  return (
    <div>
      <form onSubmit={submit}>
        <fieldset>
          <legend>{t('member.legend')}</legend>
          {create ? (
            <OptionSelect
              id={`member-${kind.name}-vocabulary`}
              label={t('column.vocabulary')}
              options={vocabularies.map((entry) => ({
                id: entry.id,
                label: entry.label === '' ? entry.prefix : entry.label,
              }))}
              value={vocabulary}
              onChange={setVocabulary}
              emptyLabel={t('member.selectVocabulary')}
            />
          ) : null}
          <TextField
            id={`member-${kind.name}-local-name`}
            label={t('detail.localName')}
            value={localName}
            onChange={setLocalName}
          />
          <TextField
            id={`member-${kind.name}-label`}
            label={t('column.label')}
            value={label}
            onChange={setLabel}
          />
          <p>
            <label htmlFor={`member-${kind.name}-comment`}>{t('column.comment')}</label>
            <textarea
              id={`member-${kind.name}-comment`}
              rows={2}
              value={comment}
              onChange={(event) => {
                setComment(event.target.value);
              }}
            />
          </p>
          {create ? (
            <OptionSelect
              id={`member-${kind.name}-owner`}
              label={t('field.owner')}
              options={userOptions(users)}
              value={owner}
              onChange={setOwner}
              emptyLabel={t('common.noOwner')}
            />
          ) : null}
        </fieldset>
        {formError === null ? null : <p role="alert">{formError}</p>}
        <SaveStatus state={saveState} />
        <SubmitRow
          label={create ? t(kind.addKey) : t('action.save')}
          busy={saveState.status === 'saving'}
        />
      </form>
    </div>
  );
};

/** `#/properties/new` and `#/resource-classes/new`. */
export const VocabularyMemberCreatePage = <MemberT extends VocabularyMemberRow>({
  kind,
}: {
  readonly kind: MemberKind<MemberT>;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(async (): Promise<{
    readonly vocabularies: readonly Vocabulary[];
    readonly users: readonly User[];
  }> => {
    const [vocabularies, users] = await Promise.all([
      api.listVocabularies({ perPage: 200, sortBy: 'label' }),
      api.listUsers({ perPage: 200, sortBy: 'name' }),
    ]);
    return { vocabularies: vocabularies.items, users: users.items };
  }, []);

  if (state.status === 'loading') {
    return <p role="status">{t(kind.loadingKey)}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <MemberEditor
      kind={kind}
      member={null}
      vocabularies={state.data.vocabularies}
      users={state.data.users}
      onSaved={() => undefined}
    />
  );
};

/**
 * The edit half of one member's detail view. `o:vocabulary` is read on create
 * only and the read carries no owner, so an update sends neither.
 */
export const VocabularyMemberEditForm = <MemberT extends VocabularyMemberRow>({
  kind,
  member,
  onSaved,
}: {
  readonly kind: MemberKind<MemberT>;
  readonly member: MemberT;
  readonly onSaved: (member: MemberT) => void;
}): ReactElement => (
  <MemberEditor kind={kind} member={member} vocabularies={[]} users={[]} onSaved={onSaved} />
);

export interface MemberDetailProps<MemberT extends VocabularyMemberRow> {
  readonly kind: MemberKind<MemberT>;
  readonly member: MemberT;
  readonly vocabulary: Vocabulary;
  readonly onSaved: (member: MemberT) => void;
}

/**
 * One member's detail view: every member the read carries, the edit form and
 * the delete control. Upstream keeps the first two on
 * `VocabularyController::showDetailsAction` and the delete on the vocabulary
 * page; here each row owns its route.
 */
export const MemberDetail = <MemberT extends VocabularyMemberRow>({
  kind,
  member,
  vocabulary,
  onSaved,
}: MemberDetailProps<MemberT>): ReactElement => {
  const { t } = useTranslator();
  return (
    <section>
      <h2>{member.label ?? member.term}</h2>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{member.id}</dd>
        <dt>{t('column.label')}</dt>
        <dd>{member.label ?? t('common.none')}</dd>
        <dt>{t('column.term')}</dt>
        <dd>{member.term}</dd>
        <dt>{t('detail.localName')}</dt>
        <dd>{member.localName}</dd>
        <dt>{t('column.comment')}</dt>
        <dd>{member.comment ?? t('common.none')}</dd>
        <dt>{t('column.vocabulary')}</dt>
        <dd>
          <a href={`#/vocabularies/${String(vocabulary.id)}`}>{vocabulary.label}</a>
        </dd>
      </dl>

      <EditPanel label={t(kind.editKey)}>
        <VocabularyMemberEditForm kind={kind} member={member} onSaved={onSaved} />
      </EditPanel>

      <DeleteControl
        label={t(kind.deleteKey)}
        rowName={member.label ?? member.term}
        onConfirm={async () => {
          await kind.remove(member.id);
          window.location.hash = `#/${kind.route}`;
        }}
      />
    </section>
  );
};
