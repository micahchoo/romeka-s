/**
 * The dials Simple's inspector screens are made of. `MasterDetail` is a list
 * of short rows beside the inspector of the selected one, stacked in one
 * column when it is narrower than 720 px. The list is a listbox: the arrow
 * keys, Home and End move the selection, and Enter moves focus into the
 * inspector. Rows that share a `group` sit under its heading, in a listbox
 * group. The selection is this component's state, because the import's
 * address names screens only. `Segmented` is one choice of a few, with the
 * answer the counts suggest marked and each answer's counts under its name.
 */
import { useId, useRef, useState } from 'react';
import type { KeyboardEvent, ReactElement, ReactNode } from 'react';

export interface MasterRow {
  readonly key: string;
  /** The row's name, which is also the option's accessible name. */
  readonly name: ReactNode;
  /** Its state and counts, read after the name. */
  readonly facts: ReactNode;
  /** The heading of the run of rows it belongs to; rows of one group are next to each other. */
  readonly group?: string;
  /** Drawn indented under the row before it. */
  readonly child?: boolean;
}

/** The rows in runs of one group, each with its index among all rows. */
const runsOf = (rows: readonly MasterRow[]): readonly { readonly group: string | undefined; readonly rows: readonly (readonly [MasterRow, number])[] }[] =>
  rows.reduce<{ group: string | undefined; rows: (readonly [MasterRow, number])[] }[]>((runs, row, index) => {
    const last = runs.at(-1);
    if (last !== undefined && last.group === row.group) {
      last.rows.push([row, index]);
    } else {
      runs.push({ group: row.group, rows: [[row, index]] });
    }
    return runs;
  }, []);

export const MasterDetail = ({
  label,
  rows,
  initial,
  children,
}: {
  readonly label: string;
  readonly rows: readonly MasterRow[];
  readonly initial: string | null;
  readonly children: (key: string) => ReactNode;
}): ReactElement => {
  const id = useId();
  const [chosen, setChosen] = useState(initial);
  const inspector = useRef<HTMLDivElement>(null);
  const selected = rows.find((row) => row.key === chosen)?.key ?? rows[0]?.key ?? null;
  const at = rows.findIndex((row) => row.key === selected);
  const optionId = (index: number): string => `${id}-${String(index)}`;

  const select = (index: number): void => {
    const row = rows[index];
    if (row !== undefined) {
      setChosen(row.key);
      document.getElementById(optionId(index))?.focus();
    }
  };
  const onKey = (event: KeyboardEvent<HTMLLIElement>): void => {
    const to = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: rows.length - 1 }[event.key];
    if (to !== undefined) {
      event.preventDefault();
      select(Math.min(rows.length - 1, Math.max(0, to)));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      inspector.current?.focus();
    }
  };

  const option = ([row, index]: readonly [MasterRow, number]): ReactElement => (
    <li
      key={row.key}
      id={optionId(index)}
      role="option"
      className={row.child === true ? 'guided-row-child' : undefined}
      aria-selected={row.key === selected}
      aria-labelledby={`${optionId(index)}-name`}
      aria-describedby={`${optionId(index)}-facts`}
      tabIndex={row.key === selected ? 0 : -1}
      onClick={() => {
        select(index);
      }}
      onKeyDown={onKey}
    >
      <span id={`${optionId(index)}-name`} className="guided-row-name">
        {row.name}
      </span>
      <span id={`${optionId(index)}-facts`} className="guided-row-facts">
        {row.facts}
      </span>
    </li>
  );

  return (
    <div className="guided-split">
      <ul role="listbox" aria-label={label} className="guided-rows">
        {runsOf(rows).map((run, at) =>
          run.group === undefined ? (
            run.rows.map(option)
          ) : (
            <li key={`group-${String(at)}`} role="none">
              <span id={`${id}-group-${String(at)}`} className="guided-group">
                {run.group}
              </span>
              <ul role="group" aria-labelledby={`${id}-group-${String(at)}`} className="guided-rows">
                {run.rows.map(option)}
              </ul>
            </li>
          ),
        )}
      </ul>
      <div ref={inspector} tabIndex={-1} className="guided-inspector">
        {selected === null ? null : children(selected)}
      </div>
    </div>
  );
};

export interface Segment<V extends string> {
  readonly value: V;
  readonly label: string;
  readonly suggested: boolean;
  /** What this answer makes, shown under its name before the click. */
  readonly note?: string;
}

/** A row of toggle buttons, one pressed: the answer the plan holds. A suggested one carries a ✓. */
export const Segmented = <V extends string>({
  label,
  segments,
  value,
  disabled,
  suggestedWord,
  onChange,
}: {
  readonly label: string;
  readonly segments: readonly Segment<V>[];
  readonly value: V | null;
  readonly disabled: boolean;
  /** What a screen reader says for the ✓. */
  readonly suggestedWord: string;
  readonly onChange: (value: V) => void;
}): ReactElement => (
  <span role="group" aria-label={label} className="guided-seg">
    {segments.map((segment) => (
      <button
        key={segment.value}
        type="button"
        aria-pressed={segment.value === value}
        disabled={disabled}
        onClick={() => {
          if (segment.value !== value) {
            onChange(segment.value);
          }
        }}
      >
        {segment.label}
        {segment.suggested ? (
          <>
            <span aria-hidden="true" className="guided-tick">
              ✓
            </span>
            {' '}
            <span className="guided-hidden">{suggestedWord}</span>
          </>
        ) : null}
        {segment.note === undefined ? null : <span className="guided-seg-note">{segment.note}</span>}
      </button>
    ))}
  </span>
);
