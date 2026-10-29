/**
 * The plain-words picker for a property or a class, and the "?" that shows a
 * term's stored definition in place. Every place that chooses a standard term
 * uses these two: the item type's fields and class, "Add another field" on
 * the item forms, and the Fields page.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import { useTermText } from '../i18n/termComments';
import { useDescribeText } from './describe-messages';
import './describe.css';
import { fieldDescription, plainName, searchTerms, termComment, type Termed } from './plain-terms';

/** "?": the term's `rdfs:comment`, shown under the control that asks. */
export const TermHelp = ({ entry }: { readonly entry: Termed }): ReactElement => {
  const t = useDescribeText();
  const text = useTermText();
  const [open, setOpen] = useState(false);
  const id = useId();
  const name = plainName(entry, text);
  const comment = termComment(entry, text);
  return (
    <span className="term-help">
      <button
        type="button"
        className="term-help-toggle"
        aria-expanded={open}
        aria-controls={id}
        aria-label={t('help.what', { name })}
        onClick={() => {
          setOpen(!open);
        }}
      >
        ?
      </button>
      <span id={id} className="term-help-text" hidden={!open}>
        <strong>{name}</strong> <code>{entry.term}</code>{' '}
        {comment === null || comment.trim() === '' ? t('help.none') : comment}
      </span>
    </span>
  );
};

export interface StandardTermPickerProps<T extends Termed> {
  readonly label: string;
  readonly entries: readonly T[];
  readonly onPick: (entry: T) => void;
  /** Offered when the search finds nothing: make a new term named by the query. */
  readonly onCreate?: ((name: string) => Promise<void>) | undefined;
  readonly placeholder?: string | undefined;
  readonly autoFocus?: boolean;
}

/** A search box and the ranked matches under it, each with its term. */
export const StandardTermPicker = <T extends Termed>({
  label,
  entries,
  onPick,
  onCreate,
  placeholder,
  autoFocus,
}: StandardTermPickerProps<T>): ReactElement => {
  const t = useDescribeText();
  const text = useTermText();
  const inputId = useId();
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const matches = searchTerms(entries, query, text);
  const typed = query.trim();
  const create =
    onCreate === undefined
      ? null
      : async (): Promise<void> => {
          setBusy(true);
          setError(null);
          try {
            await onCreate(typed);
            setQuery('');
          } catch (caught) {
            setError(caught instanceof Error ? caught.message : String(caught));
          } finally {
            setBusy(false);
          }
        };
  return (
    <div className="standard-term-picker">
      <p>
        <label htmlFor={inputId}>{label}</label>
        <input
          id={inputId}
          type="search"
          placeholder={placeholder}
          // The picker opens on a click that asked for it, so focus follows.
          autoFocus={autoFocus}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
      </p>
      {typed === '' ? null : (
        <ul className="term-matches" aria-label={t('picker.matches', { query: typed })}>
          {matches.map((entry) => (
            <li key={entry.term}>
              <button
                type="button"
                onClick={() => {
                  onPick(entry);
                  setQuery('');
                }}
              >
                <span className="term-match-name">{plainName(entry, text)}</span>
                <span className="term-match-hint">
                  {fieldDescription(entry, text) ?? ''}
                </span>
                <code className="term-match-term">{entry.term}</code>
              </button>
            </li>
          ))}
          {matches.length > 0 ? null : (
            <li className="term-match-none">{t('picker.none', { query: typed })}</li>
          )}
          {create === null ? null : (
            <li>
              <button
                type="button"
                className="term-match-create"
                disabled={busy}
                onClick={() => {
                  void create();
                }}
              >
                {t('picker.create', { name: typed })}
              </button>
            </li>
          )}
        </ul>
      )}
      {error === null ? null : <p role="alert">{error}</p>}
    </div>
  );
};
