/**
 * The filter row over an import list. A choice applies when it changes; the
 * text applies on Enter or Search, so typing never reloads the list. The
 * value is the filter a "change all of them" carries, so a change acts on
 * exactly what the list shows (`docs/import-v2.md`: act on what a filter
 * selects).
 */
import { useId, useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { useImportText } from './messages';

export type FilterField<K extends string> =
  | { readonly key: K; readonly kind: 'text'; readonly label: string }
  | {
      readonly key: K;
      readonly kind: 'choice';
      readonly label: string;
      /** The first option, which does not narrow ("Any folder"). */
      readonly any: string;
      readonly options: readonly { readonly value: string; readonly label: string }[];
    };

/** Only the fields that narrow: an empty one is absent. */
export type FilterValue<K extends string> = { readonly [P in K]?: string };

export interface FiltersProps<K extends string> {
  /** The form's name, such as "Filter the items". */
  readonly label: string;
  readonly fields: readonly FilterField<K>[];
  readonly value: FilterValue<K>;
  readonly onChange: (value: FilterValue<K>) => void;
}

const withField = <K extends string>(value: FilterValue<K>, key: K, text: string): FilterValue<K> => {
  const next: { [P in K]?: string } = {};
  for (const [name, kept] of Object.entries<string | undefined>(value)) {
    if (name !== key && kept !== undefined) {
      Object.assign(next, { [name]: kept });
    }
  }
  if (text.trim() !== '') {
    next[key] = text;
  }
  return next;
};

export const Filters = <K extends string>({ label, fields, value, onChange }: FiltersProps<K>): ReactElement => {
  const { t } = useImportText();
  const id = useId();
  const [typed, setTyped] = useState<FilterValue<K>>({});
  const textOf = (key: K): string => typed[key] ?? value[key] ?? '';
  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    let next = value;
    for (const field of fields) {
      if (field.kind === 'text') {
        next = withField(next, field.key, textOf(field.key));
      }
    }
    setTyped({});
    onChange(next);
  };
  const narrowed = Object.keys(value).length > 0;
  return (
    <form role="search" aria-label={label} className="import-filters" onSubmit={submit}>
      {fields.map((field) => {
        const control = `${id}-${field.key}`;
        return (
          <label key={field.key} htmlFor={control}>
            <span>{field.label}</span>
            {field.kind === 'text' ? (
              <input
                id={control}
                type="search"
                value={textOf(field.key)}
                onChange={(event) => {
                  setTyped({ ...typed, [field.key]: event.target.value });
                }}
              />
            ) : (
              <select
                id={control}
                value={value[field.key] ?? ''}
                onChange={(event) => {
                  onChange(withField(value, field.key, event.target.value));
                }}
              >
                <option value="">{field.any}</option>
                {field.options.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            )}
          </label>
        );
      })}
      {fields.some((field) => field.kind === 'text') ? <button type="submit">{t('paged.search')}</button> : null}
      {narrowed ? (
        <button
          type="button"
          className="link-button"
          onClick={() => {
            setTyped({});
            onChange({});
          }}
        >
          {t('paged.showAll')}
        </button>
      ) : null}
    </form>
  );
};
