/**
 * The paged table every import list uses: the past imports, and the records,
 * files and rows the review screens show. It draws one page of at most
 * `PER_PAGE` rows whatever the total, so a list of 40,000 records costs what a
 * list of 50 does (`docs/import-v2.md`, "What the admin screens need at
 * scale").
 *
 * The page in hand stays on screen while the next one loads, marked busy, so
 * paging never empties the table; a load that takes over a second says so.
 */
import { useEffect, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { Paged } from '../../api/import-sections';
import { ErrorNotice } from '../../app/errors';
import { Pager } from '../../app/list';
import type { Column } from '../../app/list';
import { TableScroll } from '../../app/table-scroll';
import { toError } from '../../app/useAsync';
import { useImportText } from './messages';

export const PER_PAGE = 50;

export interface PagedListProps<T> {
  /** The table's name, for the scroll region and the caption. */
  readonly label: string;
  /** The plural the pager counts in ("imports"). */
  readonly resource: string;
  /** One page, 1-based. */
  readonly load: (page: number, perPage: number) => Promise<Paged<T>>;
  /** What the load reads besides the page, such as a filter: a change goes back to page 1. */
  readonly deps: readonly unknown[];
  readonly columns: readonly Column<T>[];
  readonly rowKey: (row: T) => string;
  /** Shown in place of the table when the list has no rows at all. */
  readonly empty: ReactNode;
}

type Shown<T> =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly error: Error }
  | { readonly status: 'ready'; readonly data: Paged<T> };

export const PagedList = <T,>({ label, resource, load, deps, columns, rowKey, empty }: PagedListProps<T>): ReactElement => {
  const { t } = useImportText();
  const [page, setPage] = useState(1);
  const [shown, setShown] = useState<Shown<T>>({ status: 'loading' });
  const [busy, setBusy] = useState(true);
  const [seen, setSeen] = useState(deps);
  if (deps.length !== seen.length || deps.some((value, index) => !Object.is(value, seen[index]))) {
    setSeen(deps);
    setPage(1);
  }

  useEffect(() => {
    let current = true;
    setBusy(true);
    load(page, PER_PAGE).then(
      (data) => {
        if (current) {
          setShown({ status: 'ready', data });
          setBusy(false);
        }
      },
      (error: unknown) => {
        if (current) {
          setShown({ status: 'error', error: toError(error) });
          setBusy(false);
        }
      },
    );
    return () => {
      current = false;
    };
    // `load` is a new closure on every render, so `deps` names what it reads.
  }, [page, ...seen]);

  if (shown.status === 'loading') {
    return (
      <p role="status" className="import-slow">
        {t('lists.loading', { page })}
      </p>
    );
  }
  if (shown.status === 'error') {
    return <ErrorNotice error={shown.error} />;
  }
  const view = shown.data;
  if (view.total === 0) {
    return <div className="import-empty">{empty}</div>;
  }
  const lastPage = Math.max(1, Math.ceil(view.total / view.perPage));
  return (
    <section className="import-list" aria-busy={busy}>
      <TableScroll label={label}>
        <table>
          <thead>
            <tr>
              {columns.map((column) => (
                <th key={column.header} scope="col" className={column.numeric === true ? 'cell-num' : undefined}>
                  {column.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {view.items.map((row) => (
              <tr key={rowKey(row)}>
                {columns.map((column) => (
                  <td key={column.header} className={column.numeric === true ? 'cell-num' : undefined}>
                    {column.cell(row)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </TableScroll>
      {lastPage > 1 ? <Pager page={view.page} lastPage={lastPage} total={view.total} label={resource} onPage={setPage} /> : null}
      {busy ? (
        <p role="status" className="import-slow">
          {t('lists.loading', { page })}
        </p>
      ) : null}
    </section>
  );
};
