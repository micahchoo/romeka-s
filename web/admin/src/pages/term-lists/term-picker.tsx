/**
 * The value input for a term list data type (`customvocab:<id>`): a type-ahead
 * combobox over the list's entries.
 *
 * The picker owns everything about the list. It reads the list by its data
 * type name, and for a list of items it reads the item set's items, so a
 * caller only says which list and receives the entry the user picked. The
 * stored value is the entry's own text: the word, the URI, or the item id.
 */
import { useId, useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import { api } from '../../api/client';
import { getTermList, termListIdFromParam } from '../../api/term-lists';
import type { TermList, TermListName } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTermListText } from './messages';

/** One entry the user can pick, in the value shape it is stored as. */
export interface TermChoice {
  readonly kind: 'literal' | 'uri' | 'resource';
  /** What the value stores: the word, the URI, or the item id. */
  readonly value: string;
  /** What the user reads. */
  readonly label: string;
}

interface Loaded {
  readonly list: TermList;
  readonly choices: readonly TermChoice[];
}

/** The most entries the open list shows; typing narrows the rest. */
const SHOWN = 50;
const ITEMS_PER_LIST = 500;

const load = async (name: TermListName): Promise<Loaded> => {
  const id = termListIdFromParam(name.slice('customvocab:'.length));
  if (id === null) {
    throw new Error(`Not a term list: ${name}`);
  }
  const list = await getTermList(api, id);
  const source = list.source;
  switch (source.kind) {
    case 'terms':
      return {
        list,
        choices: source.terms.map((term) => ({ kind: 'literal', value: term, label: term })),
      };
    case 'uris':
      return {
        list,
        choices: source.uris.map((entry) => ({
          kind: 'uri',
          value: entry.uri,
          label: entry.label === '' ? entry.uri : entry.label,
        })),
      };
    case 'items': {
      const items = await api.listItems({
        itemSetId: source.itemSet.id,
        perPage: ITEMS_PER_LIST,
        sortBy: 'title',
      });
      return {
        list,
        choices: items.items.map((item) => ({
          kind: 'resource',
          value: String(item.id),
          label: item.title ?? `#${String(item.id)}`,
        })),
      };
    }
    case 'empty':
      return { list, choices: [] };
  }
};

/** Case and accent folded, so "du" finds "Dún Laoghaire". */
const fold = (text: string): string =>
  text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

const matching = (choices: readonly TermChoice[], query: string): readonly TermChoice[] => {
  const needle = fold(query.trim());
  return needle === ''
    ? choices
    : choices.filter((choice) => fold(choice.label).includes(needle));
};

export interface TermPickerProps {
  /** The input's id; the caller's other fields key off the same prefix. */
  readonly id: string;
  /** The element naming the field the value belongs to, read before the list's name. */
  readonly fieldLabelId?: string;
  readonly termList: TermListName;
  /** The stored value: the word, the URI, or the item id. Empty for none. */
  readonly value: string;
  /** The picked entry, or `null` when the user cleared the field. */
  readonly onPick: (choice: TermChoice | null) => void;
}

export const TermPicker = (props: TermPickerProps): ReactElement => {
  const tl = useTermListText();
  const state = useAsync(() => load(props.termList), [props.termList]);
  if (state.status === 'loading') {
    return <p role="status">{tl('termPicker.loading')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <Combobox {...props} loaded={state.data} />;
};

const Combobox = ({
  id,
  fieldLabelId,
  value,
  onPick,
  loaded,
}: TermPickerProps & { readonly loaded: Loaded }): ReactElement => {
  const tl = useTermListText();
  const listboxId = useId();
  const hintId = useId();
  const labelId = useId();
  // `null` while the user is not typing: the field then shows the stored entry.
  const [query, setQuery] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);

  const current = loaded.choices.find((choice) => choice.value === value) ?? null;
  const shown = matching(loaded.choices, query ?? '').slice(0, SHOWN);
  const activeChoice = shown[Math.min(active, shown.length - 1)];

  const pick = (choice: TermChoice | null): void => {
    onPick(choice);
    setQuery(null);
    setOpen(false);
  };

  const settle = (): void => {
    setOpen(false);
    if (query === null) {
      return;
    }
    const typed = fold(query.trim());
    if (typed === '') {
      pick(null);
      return;
    }
    const exact = loaded.choices.find((choice) => fold(choice.label) === typed);
    if (exact === undefined) {
      setQuery(null);
    } else {
      pick(exact);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      setOpen(true);
      const step = event.key === 'ArrowDown' ? 1 : -1;
      setActive((index) => Math.max(0, Math.min(shown.length - 1, index + step)));
    } else if (event.key === 'Enter' && open && activeChoice !== undefined) {
      event.preventDefault();
      pick(activeChoice);
    } else if (event.key === 'Escape' && open) {
      event.preventDefault();
      setQuery(null);
      setOpen(false);
    }
  };

  const optionId = (index: number): string => `${listboxId}-${String(index)}`;
  const expanded = open && shown.length > 0;

  return (
    <div className="value-field-wide term-picker">
      <label id={labelId} htmlFor={id}>
        {tl('termPicker.typeLabel', { list: loaded.list.label })}
      </label>
      <input
        id={id}
        type="text"
        role="combobox"
        autoComplete="off"
        aria-autocomplete="list"
        aria-expanded={expanded}
        aria-controls={listboxId}
        aria-labelledby={fieldLabelId === undefined ? undefined : `${fieldLabelId} ${labelId}`}
        aria-describedby={hintId}
        aria-activedescendant={
          expanded && activeChoice !== undefined ? optionId(shown.indexOf(activeChoice)) : undefined
        }
        value={query ?? current?.label ?? value}
        onChange={(event) => {
          setQuery(event.target.value);
          setActive(0);
          setOpen(true);
        }}
        onFocus={() => {
          setOpen(true);
        }}
        onBlur={settle}
        onKeyDown={onKeyDown}
      />
      <span id={hintId} className="field-hint">
        {tl('termPicker.hint', { list: loaded.list.label })}
      </span>
      <ul id={listboxId} role="listbox" className="term-picker-options" hidden={!expanded}>
        {shown.map((choice, index) => (
          <li
            key={choice.value}
            id={optionId(index)}
            role="option"
            aria-selected={choice === activeChoice}
            onMouseDown={(event) => {
              // Keep focus in the input so the blur does not settle first.
              event.preventDefault();
              pick(choice);
            }}
          >
            {choice.label}
          </li>
        ))}
      </ul>
      {open && query !== null && shown.length === 0 ? (
        <span className="field-hint" role="status">
          {tl('termPicker.noMatch', { query })}
        </span>
      ) : null}
      {value !== '' && current === null && query === null ? (
        <span className="field-hint" role="note">
          {tl('termPicker.notInList', { value })}
        </span>
      ) : null}
    </div>
  );
};
