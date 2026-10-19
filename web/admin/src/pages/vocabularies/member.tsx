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
import { useTermText } from '../../i18n/termComments';
import type { KeyOf, MessageKey } from '../../i18n';
import { standardsHref } from '../standards/routes';
import { common } from '../../i18n/namespaces/common';
import { standards } from '../../i18n/namespaces/standards';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof standards>;

export type VocabularyMemberRow = Property | ResourceClass;

/** One member adapter: its resource name, its Standards tab and its endpoint calls. */
export interface MemberKind<MemberT extends VocabularyMemberRow> {
  readonly name: 'properties' | 'resource_classes';
  readonly tab: 'properties' | 'classes';
  readonly addKey: MessageId;
  readonly editKey: MessageId;
  readonly deleteKey: MessageId;
  readonly loadingKey: MessageId;
  readonly create: (write: VocabularyMemberWrite) => Promise<MemberT>;
  readonly update: (id: MemberT['id'], write: VocabularyMemberWrite) => Promise<MemberT>;
  readonly remove: (id: MemberT['id']) => Promise<void>;
}

export const PROPERTY_KIND: MemberKind<Property> = {
  name: 'properties',
  tab: 'properties',
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
  tab: 'classes',
  addKey: 'resourceClasses.add',
  editKey: 'resourceClasses.editLabel',
  deleteKey: 'resourceClasses.deleteLabel',
  loadingKey: 'loading.resourceClassForm',
  create: (write) => api.createResourceClass(write),
  update: (id, write) => api.updateResourceClass(id, write),
  remove: (id) => api.deleteResourceClass(id),
};

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

/**
 * What the editor writes. `create` picks the vocabulary and the owner, `add`
 * joins the vocabulary whose page it sits on, and `edit` sends neither, since
 * `o:vocabulary` is read on create only and the read carries no owner.
 */
export type MemberTarget<MemberT extends VocabularyMemberRow> =
  | {
      readonly kind: 'create';
      readonly vocabularies: readonly Vocabulary[];
      readonly users: readonly User[];
    }
  | { readonly kind: 'add'; readonly vocabulary: VocabularyId }
  | { readonly kind: 'edit'; readonly member: MemberT };

interface MemberEditorProps<MemberT extends VocabularyMemberRow> {
  readonly kind: MemberKind<MemberT>;
  readonly target: MemberTarget<MemberT>;
  readonly onSaved: (member: MemberT) => void;
}

/** The vocabulary a write names, or `null` when a create has none chosen yet. */
const vocabularyOf = <MemberT extends VocabularyMemberRow>(
  target: MemberTarget<MemberT>,
  chosen: VocabularyId | null,
): VocabularyId | null => {
  switch (target.kind) {
    case 'create':
      return chosen;
    case 'add':
      return target.vocabulary;
    case 'edit':
      return target.member.vocabulary.id;
  }
};

const MemberEditor = <MemberT extends VocabularyMemberRow>({
  kind,
  target,
  onSaved,
}: MemberEditorProps<MemberT>): ReactElement => {
  const { t } = useTranslator(common, standards);
  const text = useTermText();
  const member = target.kind === 'edit' ? target.member : null;
  const [localName, setLocalName] = useState(member?.localName ?? '');
  const [label, setLabel] = useState(member?.label ?? '');
  const [comment, setComment] = useState(member?.comment ?? '');
  const [owner, setOwner] = useState<UserId | null>(null);
  const [vocabulary, setVocabulary] = useState<VocabularyId | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();
  const idBase = `member-${kind.name}`;

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
    const vocabularyId = vocabularyOf(target, vocabulary);
    if (vocabularyId === null) {
      setFormError(t('member.errorEmptyVocabulary'));
      return;
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
      onSaved(
        member === null ? await kind.create(write) : await kind.update(member.id, write),
      );
    });
  };

  return (
    <div>
      <form onSubmit={submit}>
        <fieldset>
          <legend>{t('member.legend')}</legend>
          {target.kind === 'create' ? (
            <OptionSelect
              id={`${idBase}-vocabulary`}
              label={t('column.vocabulary')}
              options={target.vocabularies.map((entry) => ({
                id: entry.id,
                label: entry.label === '' ? entry.prefix : text.label(entry.prefix, entry.label),
              }))}
              value={vocabulary}
              onChange={setVocabulary}
              emptyLabel={t('member.selectVocabulary')}
            />
          ) : null}
          <TextField
            id={`${idBase}-local-name`}
            label={t('detail.localName')}
            value={localName}
            onChange={setLocalName}
          />
          <TextField
            id={`${idBase}-label`}
            label={t('column.label')}
            value={label}
            onChange={setLabel}
          />
          <p>
            <label htmlFor={`${idBase}-comment`}>{t('column.comment')}</label>
            <textarea
              id={`${idBase}-comment`}
              rows={2}
              value={comment}
              onChange={(event) => {
                setComment(event.target.value);
              }}
            />
          </p>
          {target.kind === 'create' ? (
            <OptionSelect
              id={`${idBase}-owner`}
              label={t('field.owner')}
              options={userOptions(target.users)}
              value={owner}
              onChange={setOwner}
              emptyLabel={t('common.noOwner')}
            />
          ) : null}
        </fieldset>
        {formError === null ? null : <p role="alert">{formError}</p>}
        <SaveStatus state={saveState} />
        <SubmitRow
          label={member === null ? t(kind.addKey) : t('action.save')}
          busy={saveState.status === 'saving'}
        />
      </form>
    </div>
  );
};

/**
 * The add form on a vocabulary's own page: the member joins that vocabulary,
 * and the form clears for the next one.
 */
export const VocabularyMemberAddForm = <MemberT extends VocabularyMemberRow>({
  kind,
  vocabulary,
  onAdded,
}: {
  readonly kind: MemberKind<MemberT>;
  readonly vocabulary: VocabularyId;
  readonly onAdded: (member: MemberT) => void;
}): ReactElement => {
  const [round, setRound] = useState(0);
  return (
    <MemberEditor
      key={round}
      kind={kind}
      target={{ kind: 'add', vocabulary }}
      onSaved={(member) => {
        onAdded(member);
        setRound(round + 1);
      }}
    />
  );
};

/** `#/standards/properties/new` and `#/standards/classes/new`. */
export const VocabularyMemberCreatePage = <MemberT extends VocabularyMemberRow>({
  kind,
}: {
  readonly kind: MemberKind<MemberT>;
}): ReactElement => {
  const { t } = useTranslator(common, standards);
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
      target={{ kind: 'create', ...state.data }}
      onSaved={(created) => {
        window.location.hash = standardsHref(kind.tab, created.id);
      }}
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
}): ReactElement => <MemberEditor kind={kind} target={{ kind: 'edit', member }} onSaved={onSaved} />;

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
  const { t } = useTranslator(common, standards);
  const text = useTermText();
  const label = text.label(member.term, member.label);
  return (
    <section>
      <h2>{label ?? member.term}</h2>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{member.id}</dd>
        <dt>{t('column.label')}</dt>
        <dd>{label ?? t('common.none')}</dd>
        <dt>{t('column.term')}</dt>
        <dd>{member.term}</dd>
        <dt>{t('detail.localName')}</dt>
        <dd>{member.localName}</dd>
        <dt>{t('column.comment')}</dt>
        <dd>{text.comment(member.term, member.comment) ?? t('common.none')}</dd>
        <dt>{t('column.vocabulary')}</dt>
        <dd>
          <a href={standardsHref('vocabularies', vocabulary.id)}>
            {text.label(vocabulary.prefix, vocabulary.label)}
          </a>
        </dd>
      </dl>

      <EditPanel label={t(kind.editKey)}>
        <VocabularyMemberEditForm kind={kind} member={member} onSaved={onSaved} />
      </EditPanel>

      <DeleteControl
        label={t(kind.deleteKey)}
        rowName={label ?? member.term}
        onConfirm={async () => {
          await kind.remove(member.id);
          window.location.hash = standardsHref(kind.tab);
        }}
      />
    </section>
  );
};
