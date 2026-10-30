/**
 * `#/fields/<id>`: the values board (`briefs/admin-values-board-design.md`). The field's texts sit
 * in the first column, most used first; each other column is a field a text can move to. Dragging
 * a text onto a column, or choosing the column under "Move to", checks the move with the server
 * (`dry_run`), says what it will do, and moves the text on every record the person may change that
 * holds it. The last move of the visit can be undone: the server's answer names the records it
 * moved and merged, which is the exact way back (`undoOf`).
 *
 * The board states only what the server counts. Which field a text belongs in is the person's call.
 */
import { useEffect, useId, useState } from 'react';
import type { DragEvent, ReactElement } from 'react';
import { api } from '../../api/client';
import { getFieldValues, moveFieldValue, recordsOf, undoOf } from '../../api/field-values';
import type { FieldValue, FieldValues, Move, Moved } from '../../api/field-values';
import { isRecord } from '../../api/parse';
import { fieldIsQuery } from '../../api/places';
import type { Property, PropertyId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { plainName } from '../../app/plain-terms';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { fieldValues } from '../../i18n/namespaces/field-values';
import { useTermText } from '../../i18n/termComments';
import { itemsHref } from '../items/browse';
import { browseOrder, load } from './field-order';
import type { Loaded } from './field-order';
import { valuesHref } from './routes';
import './values-board.css';

/** The fields a source column starts with beside it, when the person has chosen none. */
const STARTING_TARGETS = ['dcterms:spatial', 'dcterms:creator', 'dcterms:medium', 'dcterms:date'];
const STARTING_COLUMNS = 3;
const PAGE = 50;
/** A target column shows its most used values only; the rest are a count. */
const TARGET_SHOWN = 12;
const DRAG_TYPE = 'application/x-romeka-values';

const storageKey = (source: PropertyId): string => `romeka.valuesBoard.columns.${String(source)}`;

/** The columns the person left beside this field, a per-browser convenience. */
const readColumns = (source: PropertyId): readonly number[] | null => {
  try {
    const raw = window.localStorage.getItem(storageKey(source));
    const parsed: unknown = raw === null ? null : JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((id): id is number => typeof id === 'number' && Number.isSafeInteger(id)) : null;
  } catch {
    return null;
  }
};

const writeColumns = (source: PropertyId, columns: readonly PropertyId[]): void => {
  try {
    window.localStorage.setItem(storageKey(source), JSON.stringify(columns));
  } catch {
    // A browser that keeps nothing starts from the default columns next time.
  }
};

const startingColumns = (loaded: Loaded, source: PropertyId): readonly PropertyId[] => {
  const known = (id: number): Property | undefined => loaded.properties.find((property) => property.id === id);
  const stored = readColumns(source);
  if (stored !== null) {
    return stored.flatMap((id) => {
      const property = known(id);
      return property === undefined || property.id === source ? [] : [property.id];
    });
  }
  return STARTING_TARGETS.flatMap((term) => {
    const property = loaded.properties.find((candidate) => candidate.term === term);
    return property === undefined || property.id === source ? [] : [property.id];
  }).slice(0, STARTING_COLUMNS);
};

/** A field's census, kept on screen while a newer one loads, so a move does not blank the board. */
const useCensus = (property: PropertyId, version: number): { data: FieldValues | null; error: Error | null } => {
  const [state, setState] = useState<{ data: FieldValues | null; error: Error | null; of: PropertyId }>({
    data: null,
    error: null,
    of: property,
  });
  useEffect(() => {
    let cancelled = false;
    getFieldValues(api, property).then(
      (data) => {
        if (!cancelled) setState({ data, error: null, of: property });
      },
      (error: unknown) => {
        if (!cancelled) setState((current) => ({ ...current, error: toError(error), of: property }));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [property, version]);
  return state.of === property ? state : { data: null, error: null };
};

interface Peeked {
  readonly id: number;
  readonly href: string;
  readonly name: string;
}

/** The first records that hold the text, read through the ordinary list search. */
const peek = async (property: PropertyId, value: FieldValue): Promise<readonly Peeked[]> => {
  const params = (): URLSearchParams => {
    const query = new URLSearchParams({ per_page: '6' });
    for (const [key, text] of Object.entries(fieldIsQuery(property, value.value))) {
      query.set(key, String(text));
    }
    return query;
  };
  const read = async (path: string, kind: 'items' | 'media' | 'item-sets'): Promise<readonly Peeked[]> => {
    const rows = await api.requestJson(path, params());
    return (Array.isArray(rows) ? rows : []).flatMap((row: unknown) => {
      if (!isRecord(row)) return [];
      const record = row;
      const id = record['o:id'];
      if (typeof id !== 'number') return [];
      const title = record['o:title'];
      const source = record['o:source'];
      const name = typeof title === 'string' && title !== '' ? title : typeof source === 'string' ? source : '';
      return [{ id, href: `#/${kind}/${String(id)}`, name }];
    });
  };
  const lists = await Promise.all([
    value.items > 0 ? read('/api/items', 'items') : [],
    value.media > 0 ? read('/api/media', 'media') : [],
    value.itemSets > 0 ? read('/api/item_sets', 'item-sets') : [],
  ]);
  return lists.flat();
};

const Peek = ({ property, value }: { readonly property: PropertyId; readonly value: FieldValue }): ReactElement => {
  const { t } = useTranslator(fieldValues, common);
  const state = useAsync(() => peek(property, value), [property, value.value]);
  if (state.status === 'loading') return <p className="values-peek values-note">{t('values.reading')}</p>;
  if (state.status === 'error') return <ErrorNotice error={state.error} />;
  return (
    <div className="values-peek">
      <ul>
        {state.data.map((record) => (
          <li key={record.href}>
            <a href={record.href}>{record.name === '' ? t('common.untitledNumber', { id: String(record.id) }) : record.name}</a>
          </li>
        ))}
      </ul>
      {value.items > 0 ? (
        <a href={itemsHref({ field: { property, presence: 'is', text: value.value } })}>{t('values.openItems')}</a>
      ) : null}
    </div>
  );
};

interface CardProps {
  readonly property: PropertyId;
  readonly value: FieldValue;
  readonly movable: boolean;
  readonly chosen: readonly string[];
  readonly targets: readonly { readonly id: PropertyId; readonly name: string }[];
  readonly onChoose?: ((value: string, chosen: boolean) => void) | undefined;
  readonly onMove?: ((values: readonly string[], to: PropertyId) => void) | undefined;
}

const ValueCard = ({ property, value, movable, chosen, targets, onChoose, onMove }: CardProps): ReactElement => {
  const { t, number } = useTranslator(fieldValues, common);
  const [open, setOpen] = useState(false);
  const total = recordsOf(value);
  const canMove = movable && value.editable > 0;
  const isChosen = chosen.includes(value.value);
  const moving = isChosen ? chosen : [value.value];
  const counts = [
    value.items === 0 ? null : value.items === 1 ? t('values.itemsOne') : t('values.itemsMany', { count: number(value.items) }),
    value.media === 0 ? null : t('values.mediaCount', { count: number(value.media) }),
    value.itemSets === 0
      ? null
      : value.itemSets === 1
        ? t('values.setsOne')
        : t('values.setsMany', { count: number(value.itemSets) }),
  ].filter((part) => part !== null);
  return (
    <li
      className={isChosen ? 'values-card values-card-chosen' : 'values-card'}
      draggable={canMove}
      onDragStart={(event: DragEvent<HTMLLIElement>) => {
        event.dataTransfer.setData(DRAG_TYPE, JSON.stringify(moving));
        event.dataTransfer.setData('text/plain', moving.join(', '));
        event.dataTransfer.effectAllowed = 'move';
      }}
    >
      <div className="values-card-head">
        {canMove ? (
          <>
            <span className="values-grip" aria-hidden="true">
              ⠿
            </span>
            <input
              type="checkbox"
              aria-label={t('values.choose', { value: value.value })}
              checked={isChosen}
              onChange={(event) => {
                onChoose?.(value.value, event.target.checked);
              }}
            />
          </>
        ) : null}
        <span className="values-text">{value.value}</span>
        <span className="values-count">{counts.join(' · ')}</span>
      </div>
      {movable && value.editable < total ? (
        <p className="values-note">
          {value.editable === 0
            ? t('values.readOnly')
            : t('values.partEditable', { editable: number(value.editable), total: number(total) })}
        </p>
      ) : null}
      <div className="values-card-actions">
        <button
          type="button"
          className="link-button"
          aria-expanded={open}
          onClick={() => {
            setOpen(!open);
          }}
        >
          {open ? t('values.hide') : t('values.show')}
        </button>
        {canMove && targets.length > 0 ? (
          <select
            aria-label={t('values.moveLabel', { value: value.value })}
            value=""
            onChange={(event) => {
              const to = targets.find((target) => String(target.id) === event.target.value);
              if (to !== undefined) onMove?.(moving, to.id);
            }}
          >
            <option value="">{t('values.moveTo')}</option>
            {targets.map((target) => (
              <option key={String(target.id)} value={String(target.id)}>
                {target.name}
              </option>
            ))}
          </select>
        ) : null}
      </div>
      {open ? <Peek property={property} value={value} /> : null}
    </li>
  );
};

const valuesOfDrop = (event: DragEvent): readonly string[] | null => {
  try {
    const parsed: unknown = JSON.parse(event.dataTransfer.getData(DRAG_TYPE));
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === 'string') : null;
  } catch {
    return null;
  }
};

interface ColumnProps {
  readonly property: PropertyId;
  readonly name: string;
  readonly version: number;
  readonly source: boolean;
  readonly chosen: readonly string[];
  readonly targets: readonly { readonly id: PropertyId; readonly name: string }[];
  readonly onChoose?: (value: string, chosen: boolean) => void;
  readonly onMove?: (values: readonly string[], to: PropertyId) => void;
  readonly onRemove?: () => void;
}

const Column = ({ property, name, version, source, chosen, targets, onChoose, onMove, onRemove }: ColumnProps): ReactElement => {
  const { t, number } = useTranslator(fieldValues, common);
  const searchId = useId();
  const headingId = useId();
  const census = useCensus(property, version);
  const [search, setSearch] = useState('');
  const [shown, setShown] = useState(PAGE);
  const [over, setOver] = useState(false);
  const needle = search.trim().toLocaleLowerCase();
  const values = (census.data?.values ?? []).filter(
    (value) => needle === '' || value.value.toLocaleLowerCase().includes(needle),
  );
  const limit = source ? shown : TARGET_SHOWN;
  const total = census.data?.values.length ?? 0;
  return (
    <section
      className={`values-column${source ? ' values-column-source' : ''}${over ? ' values-column-over' : ''}`}
      aria-labelledby={headingId}
      onDragOver={(event) => {
        if (!source && event.dataTransfer.types.includes(DRAG_TYPE)) {
          event.preventDefault();
          setOver(true);
        }
      }}
      onDragLeave={() => {
        setOver(false);
      }}
      onDrop={(event) => {
        setOver(false);
        const dropped = valuesOfDrop(event);
        if (!source && dropped !== null) {
          event.preventDefault();
          onMove?.(dropped, property);
        }
      }}
    >
      <header className="values-column-head">
        <h3 id={headingId}>{source ? name : <a href={valuesHref(property)}>{name}</a>}</h3>
        {onRemove === undefined ? null : (
          <button
            type="button"
            className="link-button values-remove"
            aria-label={t('values.removeColumn', { field: name })}
            onClick={onRemove}
          >
            ×
          </button>
        )}
      </header>
      {census.error !== null ? <ErrorNotice error={census.error} /> : null}
      {census.data === null ? (
        <p className="values-note">{t('values.loading')}</p>
      ) : (
        <p className="values-note">{total === 1 ? t('values.countOne') : t('values.countMany', { count: number(total) })}</p>
      )}
      {source ? (
        <p className="values-search">
          <label htmlFor={searchId}>{t('values.find')}</label>
          <input
            id={searchId}
            type="search"
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setShown(PAGE);
            }}
          />
        </p>
      ) : null}
      {census.data !== null && values.length === 0 ? (
        <p className="values-empty">
          {needle !== ''
            ? t('values.noMatch', { text: search.trim() })
            : source
              ? t('values.empty', { field: name })
              : t('values.dropHere', { field: name })}
        </p>
      ) : (
        <ul className="values-cards">
          {values.slice(0, limit).map((value) => (
            <ValueCard
              key={value.value}
              property={property}
              value={value}
              movable={source}
              chosen={chosen}
              targets={targets}
              onChoose={onChoose}
              onMove={onMove}
            />
          ))}
        </ul>
      )}
      {values.length > limit ? (
        source ? (
          <button
            type="button"
            onClick={() => {
              setShown(shown + PAGE);
            }}
          >
            {t('values.showMore', { count: number(Math.min(PAGE, values.length - limit)) })}
          </button>
        ) : (
          <p className="values-note">{t('values.andMore', { count: number(values.length - limit) })}</p>
        )
      ) : null}
    </section>
  );
};

interface Pending {
  readonly values: readonly string[];
  readonly to: PropertyId;
}

type Check =
  | { readonly status: 'checking' }
  | { readonly status: 'ready'; readonly answers: readonly { readonly move: Move; readonly answer: Moved }[] }
  | { readonly status: 'moving' }
  | { readonly status: 'error'; readonly error: Error };

interface Done {
  readonly move: Move;
  readonly answer: Moved;
}

const MoveCheck = ({
  check,
  name,
  onConfirm,
  onCancel,
}: {
  readonly check: Check;
  readonly name: (id: PropertyId) => string;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const { t, number } = useTranslator(fieldValues, common);
  const headingId = useId();
  const anything = check.status === 'ready' && check.answers.some(({ answer }) => answer.changed > 0);
  return (
    <section className="values-check" role="region" aria-labelledby={headingId} aria-live="polite">
      <h3 id={headingId}>{t('values.checkTitle')}</h3>
      {check.status === 'checking' ? <p>{t('values.checking')}</p> : null}
      {check.status === 'moving' ? <p>{t('values.moving')}</p> : null}
      {check.status === 'error' ? <ErrorNotice error={check.error} /> : null}
      {check.status === 'ready'
        ? check.answers.map(({ move, answer }) => {
            const words = { value: move.value, from: name(move.from), to: name(move.to) };
            return (
              <div key={move.value} className="values-check-move">
                <p>
                  {answer.changed === 0
                    ? t('values.nothing', words)
                    : answer.changed === 1
                      ? t('values.previewOne', words)
                      : t('values.previewMany', { ...words, count: number(answer.changed) })}
                </p>
                {answer.merged.length > 0 ? (
                  <p className="values-note">{t('values.merged', { ...words, count: number(answer.merged.length) })}</p>
                ) : null}
                {answer.skipped > 0 ? (
                  <p className="values-note">{t('values.skipped', { ...words, count: number(answer.skipped) })}</p>
                ) : null}
                {answer.notInForm.map((type) => (
                  <p key={type.template} className="values-warning">
                    {t('values.notInForm', { ...words, type: type.label, count: number(type.resources) })}{' '}
                    <a href={`#/resource-templates/${String(type.template)}`}>
                      {t('values.addToType', { to: words.to, type: type.label })}
                    </a>
                  </p>
                ))}
              </div>
            );
          })
        : null}
      <p className="values-check-actions">
        <button type="button" className="button-primary" disabled={!anything} onClick={onConfirm}>
          {t('values.move')}
        </button>{' '}
        <button type="button" onClick={onCancel} disabled={check.status === 'moving'}>
          {t('action.cancel')}
        </button>
      </p>
    </section>
  );
};

const Board = ({ source, loaded }: { readonly source: Property; readonly loaded: Loaded }): ReactElement => {
  const { t, number } = useTranslator(fieldValues, common);
  const text = useTermText();
  const addId = useId();
  const name = (id: PropertyId): string => {
    const property = loaded.properties.find((candidate) => candidate.id === id);
    return property === undefined ? String(id) : plainName(property, text);
  };
  const [columns, setColumns] = useState<readonly PropertyId[]>(() => startingColumns(loaded, source.id));
  const [version, setVersion] = useState(0);
  const [chosen, setChosen] = useState<readonly string[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [check, setCheck] = useState<Check>({ status: 'checking' });
  const [done, setDone] = useState<readonly Done[]>([]);
  const [notice, setNotice] = useState('');
  const [undoing, setUndoing] = useState(false);
  const [undoError, setUndoError] = useState<Error | null>(null);

  const changeColumns = (next: readonly PropertyId[]): void => {
    setColumns(next);
    writeColumns(source.id, next);
  };
  const targets = columns.map((id) => ({ id, name: name(id) }));

  useEffect(() => {
    if (pending === null) return;
    let cancelled = false;
    setCheck({ status: 'checking' });
    const moves: readonly Move[] = pending.values.map((value) => ({ value, from: source.id, to: pending.to }));
    Promise.all(moves.map(async (move) => ({ move, answer: await moveFieldValue(api, move, true) }))).then(
      (answers) => {
        if (!cancelled) setCheck({ status: 'ready', answers });
      },
      (error: unknown) => {
        if (!cancelled) setCheck({ status: 'error', error: toError(error) });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [pending, source.id]);

  const confirm = async (): Promise<void> => {
    if (check.status !== 'ready' || pending === null) return;
    const moves = check.answers.filter(({ answer }) => answer.changed > 0).map(({ move }) => move);
    setCheck({ status: 'moving' });
    try {
      const finished: Done[] = [];
      for (const move of moves) {
        finished.push({ move, answer: await moveFieldValue(api, move, false) });
      }
      const records = finished.reduce((sum, { answer }) => sum + answer.changed, 0);
      const first = finished[0];
      setDone([...done, ...finished]);
      setNotice(
        finished.length === 1 && first !== undefined
          ? t('values.doneOne', { value: first.move.value, to: name(pending.to), count: number(records) })
          : t('values.doneMany', { values: number(finished.length), to: name(pending.to), count: number(records) }),
      );
      setChosen([]);
      setPending(null);
      setVersion(version + 1);
    } catch (error) {
      setCheck({ status: 'error', error: toError(error) });
    }
  };

  const undoLast = async (): Promise<void> => {
    const last = done[done.length - 1];
    if (last === undefined) return;
    setUndoing(true);
    setUndoError(null);
    try {
      await moveFieldValue(api, undoOf(last.move, last.answer), false);
      setDone(done.slice(0, -1));
      setNotice(t('values.undone', { value: last.move.value, from: name(last.move.from) }));
      setVersion(version + 1);
    } catch (error) {
      setUndoError(toError(error));
    } finally {
      setUndoing(false);
    }
  };

  const addable = browseOrder(loaded, text).filter(
    (property) => property.id !== source.id && !columns.includes(property.id),
  );

  return (
    <section className="values-board">
      <p>
        <a href="#/fields">{t('values.allFields')}</a>
      </p>
      <h2>{t('values.title', { field: plainName(source, text) })}</h2>
      <p className="type-lede">{t('values.intro')}</p>
      <div className="values-toolbar">
        <p className="values-add">
          <label htmlFor={addId}>{t('values.addColumn')}</label>
          <select
            id={addId}
            value=""
            onChange={(event) => {
              const property = addable.find((candidate) => String(candidate.id) === event.target.value);
              if (property !== undefined) changeColumns([...columns, property.id]);
            }}
          >
            <option value="">{t('values.chooseField')}</option>
            {addable.map((property) => (
              <option key={String(property.id)} value={String(property.id)}>
                {plainName(property, text)} ({property.term})
              </option>
            ))}
          </select>
        </p>
        {chosen.length > 0 ? (
          <p className="values-chosen" role="status">
            {t('values.chosen', { count: number(chosen.length) })}{' '}
            <button
              type="button"
              className="link-button"
              onClick={() => {
                setChosen([]);
              }}
            >
              {t('values.clearChosen')}
            </button>
          </p>
        ) : null}
      </div>
      {pending === null ? null : (
        <MoveCheck
          check={check}
          name={name}
          onConfirm={() => {
            void confirm();
          }}
          onCancel={() => {
            setPending(null);
          }}
        />
      )}
      {notice === '' ? null : (
        <p className="values-done" role="status">
          {notice}{' '}
          {done.length > 0 ? (
            <button
              type="button"
              className="link-button"
              disabled={undoing}
              onClick={() => {
                void undoLast();
              }}
            >
              {undoing ? t('values.undoing') : t('values.undo')}
            </button>
          ) : null}
        </p>
      )}
      {undoError === null ? null : <ErrorNotice error={undoError} />}
      <div className="values-columns">
        <Column
          property={source.id}
          name={plainName(source, text)}
          version={version}
          source
          chosen={chosen}
          targets={targets}
          onChoose={(value, isChosen) => {
            setChosen(isChosen ? [...chosen, value] : chosen.filter((other) => other !== value));
          }}
          onMove={(values, to) => {
            setPending({ values, to });
          }}
        />
        {targets.map((target) => (
          <Column
            key={String(target.id)}
            property={target.id}
            name={target.name}
            version={version}
            source={false}
            chosen={[]}
            targets={[]}
            onMove={(values, to) => {
              setPending({ values, to });
            }}
            onRemove={() => {
              changeColumns(columns.filter((id) => id !== target.id));
            }}
          />
        ))}
      </div>
      {done.length === 0 ? null : (
        <aside className="values-history">
          <h3>{t('values.history')}</h3>
          <ol>
            {done.map(({ move, answer }, index) => (
              <li key={`${move.value}-${String(index)}`}>
                {t('values.historyRow', {
                  value: move.value,
                  from: name(move.from),
                  to: name(move.to),
                  count: number(answer.changed),
                })}
              </li>
            ))}
          </ol>
          <button
            type="button"
            disabled={undoing}
            onClick={() => {
              void undoLast();
            }}
          >
            {t('values.undoLast')}
          </button>
          <p className="values-note">{t('values.undoNote')}</p>
        </aside>
      )}
    </section>
  );
};

export const ValuesBoard = ({ property }: { readonly property: PropertyId }): ReactElement => {
  const { t } = useTranslator(fieldValues);
  const state = useAsync(load, []);
  if (state.status === 'loading') return <p role="status">{t('values.loading')}</p>;
  if (state.status === 'error') return <ErrorNotice error={state.error} />;
  const source = state.data.properties.find((candidate) => candidate.id === property);
  if (source === undefined) {
    return <p role="alert">{t('values.noField', { id: String(property) })}</p>;
  }
  return <Board key={String(property)} source={source} loaded={state.data} />;
};
