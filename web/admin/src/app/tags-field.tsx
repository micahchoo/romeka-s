/**
 * The tags of the Subject field on the item, item set and media forms: free
 * words a reader adds while describing, stored as plain-text `dcterms:subject`
 * values so any Omeka S client shows them. They sit in the Subject field among
 * its other values, so the form, the read list and the write keep Subject in
 * one place under one name. A comma or Enter adds a tag, and the tags already
 * in the archive are offered as you type.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useId, useState } from 'react';
import { api } from '../api/client';
import { TAG_TERM, TAG_TYPE, getCensus } from '../api/tags';
import { useDescribeText } from './describe-messages';
import { draftKey, isBlank, isTag } from './draft';
import type { ResourceDraft, ValueDraft } from './draft';
import { CORE_TYPES, type PropertyField } from './value-fields';
import './tags.css';

/** The field whose plain-text values are tags. */
export const isTagField = (field: Pick<PropertyField, 'term'>): boolean => field.term === TAG_TERM;

/** The tags of the field: its plain-text values with something typed. */
export const tagsOf = (field: PropertyField): readonly ValueDraft[] =>
  field.values.filter((value) => isTag(value) && !isBlank(value));

/**
 * The type "Add value" gives the field's other values: the first it accepts
 * that is not plain text, or none when it accepts only plain text.
 */
export const otherValueType = (field: PropertyField): string | undefined =>
  (field.rule === null || field.rule.dataTypes.length === 0 ? CORE_TYPES : field.rule.dataTypes).find(
    (type) => type !== TAG_TYPE,
  );

/** The draft with these words added as tags of the field, skipping blanks and repeats. */
export const addTags = (draft: ResourceDraft, field: PropertyField, words: readonly string[]): ResourceDraft => {
  const added: ValueDraft[] = [];
  const known = new Set(tagsOf(field).map((tag) => tag.text));
  for (const word of words.map((entry) => entry.trim())) {
    if (word !== '' && !known.has(word)) {
      known.add(word);
      added.push({
        key: draftKey(),
        propertyId: field.propertyId,
        term: field.term,
        type: TAG_TYPE,
        kind: 'literal',
        text: word,
        label: '',
        lang: field.rule?.defaultLang ?? '',
        isPublic: !(field.rule?.isPrivate ?? false),
      });
    }
  }
  return added.length === 0 ? draft : { ...draft, values: [...draft.values, ...added] };
};

export const SubjectTags = ({
  inputId,
  labelId,
  field,
  draft,
  onChange,
}: {
  readonly inputId: string;
  /** The field's name, which names the group. */
  readonly labelId: string;
  readonly field: PropertyField;
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
}): ReactElement => {
  const d = useDescribeText();
  const [typed, setTyped] = useState('');
  const [known, setKnown] = useState<readonly string[] | null>(null);
  const listId = useId();
  const hintId = `${inputId}-hint`;
  const tags = tagsOf(field);

  const commit = (words: readonly string[]): void => {
    onChange(addTags(draft, field, words));
  };
  const loadKnown = (): void => {
    if (known !== null) {
      return;
    }
    setKnown([]);
    getCensus(api, { kind: 'tags' }).then(
      (census) => {
        setKnown(census.values.map((count) => count.value));
      },
      () => undefined,
    );
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === 'Enter' || event.key === ',') {
      event.preventDefault();
      commit([typed]);
      setTyped('');
    }
  };
  const onType = (text: string): void => {
    const parts = text.split(',');
    const rest = parts.pop() ?? '';
    if (parts.length > 0) {
      commit(parts);
    }
    setTyped(rest);
  };
  const offered = (known ?? []).filter((word) => !tags.some((tag) => tag.text === word));

  return (
    <div className="tags-field" role="group" aria-labelledby={labelId}>
      {tags.length === 0 ? null : (
        <ul className="tag-chips">
          {tags.map((tag) => (
            <li key={tag.key}>
              <span>{tag.text}</span>
              <button
                type="button"
                className="tag-remove"
                aria-label={d('tags.remove', { tag: tag.text })}
                onClick={() => {
                  onChange({ ...draft, values: draft.values.filter((entry) => entry.key !== tag.key) });
                }}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
      <label htmlFor={inputId}>{d('tags.add')}</label>
      <input
        id={inputId}
        type="text"
        list={listId}
        autoComplete="off"
        aria-describedby={hintId}
        value={typed}
        onFocus={loadKnown}
        onKeyDown={onKeyDown}
        onChange={(event) => {
          const native = event.nativeEvent;
          // A pick from the suggestions arrives as a replacement, not typing.
          const picked =
            !(native instanceof InputEvent) || native.inputType === 'insertReplacementText';
          if (picked && offered.includes(event.target.value)) {
            commit([event.target.value]);
            setTyped('');
            return;
          }
          onType(event.target.value);
        }}
        onBlur={() => {
          if (typed.trim() !== '') {
            commit([typed]);
            setTyped('');
          }
        }}
      />
      <datalist id={listId}>
        {offered.map((word) => (
          <option key={word} value={word} />
        ))}
      </datalist>
      <span id={hintId} className="field-hint">
        {d('tags.hint')}
      </span>
    </div>
  );
};
