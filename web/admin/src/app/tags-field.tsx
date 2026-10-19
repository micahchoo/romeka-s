/**
 * The Tags field of the item, item set and media forms: free words a reader
 * adds while describing, stored as plain-text `dcterms:subject` values so any
 * Omeka S client shows them. It needs no setup; a comma or Enter adds a tag,
 * and the tags already in the archive are offered as you type.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useId, useState } from 'react';
import { api } from '../api/client';
import { TAG_TERM, TAG_TYPE, getCensus } from '../api/tags';
import type { Property } from '../api/types';
import { useDescribeText } from './describe-messages';
import { draftKey } from './draft';
import type { ResourceDraft } from './draft';
import './tags.css';

/** The draft with these words added as tags, skipping blanks and repeats. */
export const addTags = (
  draft: ResourceDraft,
  subject: Pick<Property, 'id'>,
  words: readonly string[],
): ResourceDraft => {
  const tags = [...draft.tags];
  for (const word of words.map((entry) => entry.trim())) {
    if (word !== '' && !tags.some((tag) => tag.text === word)) {
      tags.push({
        key: draftKey(),
        propertyId: subject.id,
        term: TAG_TERM,
        type: TAG_TYPE,
        kind: 'literal',
        text: word,
        label: '',
        lang: '',
        isPublic: true,
      });
    }
  }
  return { ...draft, tags };
};

export const TagsField = ({
  idPrefix,
  draft,
  onChange,
  properties,
}: {
  readonly idPrefix: string;
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
  readonly properties: readonly Property[];
}): ReactElement | null => {
  const d = useDescribeText();
  const [typed, setTyped] = useState('');
  const [known, setKnown] = useState<readonly string[] | null>(null);
  const listId = useId();
  const subject = properties.find((property) => property.term === TAG_TERM);
  if (subject === undefined) {
    return null;
  }
  const inputId = `${idPrefix}-tags`;
  const hintId = `${inputId}-hint`;

  const commit = (words: readonly string[]): void => {
    onChange(addTags(draft, subject, words));
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
  const offered = (known ?? []).filter((word) => !draft.tags.some((tag) => tag.text === word));

  return (
    <fieldset className="tags-field">
      <legend>{d('tags.legend')}</legend>
      {draft.tags.length === 0 ? null : (
        <ul className="tag-chips">
          {draft.tags.map((tag) => (
            <li key={tag.key}>
              <span>{tag.text}</span>
              <button
                type="button"
                className="tag-remove"
                aria-label={d('tags.remove', { tag: tag.text })}
                onClick={() => {
                  onChange({ ...draft, tags: draft.tags.filter((entry) => entry !== tag) });
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
    </fieldset>
  );
};
