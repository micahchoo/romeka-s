/**
 * Step 5, Grid: one row per item as the server derives it, 50 at a time.
 * The browser holds one page and the plan; every fix, edit, skip and mapping
 * is a plan edit, and the next preview shows its effect.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { PreviewRequest, RowsPreview, RowsQuery } from '../../api/import-preview';
import type { ImportPlan } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { useTermLabels } from '../../i18n/termText';
import { collectRows } from './collect-rows';
import type { RowSelection } from './collect-rows';
import { downloadText, gridCsv } from './grid-csv';
import { EditBar } from './grid-edit';
import { SKIP } from './grid-fixes';
import type { Fix } from './grid-fixes';
import { ProblemKinds, sameKind, useKindText } from './grid-kinds';
import type { KindEntry, KindFilter } from './grid-kinds';
import { FillPanel, MapPanel } from './grid-panels';
import { GridTable } from './grid-table';
import type { CellRef } from './grid-table';
import { useImportText } from './messages';
import { withColumn, withEdit, withIncluded, withSkipped, withoutEdit } from './plan-changes';
import { PreviewError } from './preview-error';
import type { TabProps } from './tabs';
import { usePreview } from './use-preview';

export const GRID_PAGE = 50;

/** What the grid shows: the filters and the page, which together are the rows query. */
interface View {
  readonly onlyProblems: boolean;
  readonly onlyExisting: boolean;
  readonly filter: KindFilter | null;
  readonly query: string;
  readonly offset: number;
}

/** What is open over the grid: at most one panel, or the edit bar on one cell. */
type Open =
  | { readonly kind: 'fill'; readonly term: string }
  | { readonly kind: 'map'; readonly header: string }
  | { readonly kind: 'edit'; readonly cell: CellRef };

const selectionOf = (filter: KindFilter | null): RowSelection =>
  filter === null ? {} : { kind: filter.kind, ...(filter.term === null ? {} : { term: filter.term }) };

const rowsQuery = (view: View): RowsQuery => ({
  problemsOnly: view.onlyProblems,
  ...(view.onlyExisting ? { existingOnly: true } : {}),
  ...selectionOf(view.filter),
  ...(view.query.trim() === '' ? {} : { query: view.query.trim() }),
  offset: view.offset,
  limit: GRID_PAGE,
});

const gridRequest = (plan: ImportPlan, view: View): PreviewRequest => ({ include: ['rows'], rows: rowsQuery(view), plan });

const Pager = ({
  rows,
  view,
  shown,
  onOffset,
}: {
  readonly rows: RowsPreview;
  readonly view: View;
  /** The offset of the page on screen, which lags `view.offset` while the next page loads. */
  readonly shown: number;
  readonly onOffset: (offset: number) => void;
}): ReactElement => {
  const { t, number } = useImportText();
  const first = rows.filtered === 0 ? 0 : shown + 1;
  const last = Math.min(rows.filtered, shown + GRID_PAGE);
  return (
    <span className="import-pager">
      <span className="query-count">
        {t('grid.range', { first: number(first), last: number(last), count: number(rows.filtered) })}
      </span>
      <button type="button" disabled={view.offset === 0} onClick={() => { onOffset(Math.max(0, view.offset - GRID_PAGE)); }}>
        {t('grid.previous')}
      </button>
      <button
        type="button"
        disabled={view.offset + GRID_PAGE >= rows.filtered}
        onClick={() => { onOffset(view.offset + GRID_PAGE); }}
      >
        {t('grid.next')}
      </button>
    </span>
  );
};

/** The plan edits that need the server's rows first: bulk fixes and the download. */
const useBulk = (): {
  readonly busy: boolean;
  readonly error: Error | null;
  readonly run: (work: () => Promise<void>) => void;
} => {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const run = (work: () => Promise<void>): void => {
    setBusy(true);
    setError(null);
    work().then(
      () => { setBusy(false); },
      (failure: unknown) => {
        setBusy(false);
        setError(toError(failure));
      },
    );
  };
  return { busy, error, run };
};

interface OverlayProps extends Pick<TabProps, 'plan' | 'editable' | 'onChange'> {
  readonly detail: TabProps['detail'];
  readonly rows: RowsPreview;
  readonly open: Open | null;
  readonly label: (term: string) => string;
  readonly onOpen: (open: Open | null) => void;
}

/** The fill panel, the Map… panel and the edit bar: at most one is open. */
const Overlays = ({ detail, plan, editable, onChange, rows, open, label, onOpen }: OverlayProps): ReactElement | null => {
  const close = (): void => { onOpen(null); };
  if (open === null) {
    return null;
  }
  if (open.kind === 'fill') {
    return (
      <FillPanel
        key={`fill:${open.term}`}
        id={detail.id}
        plan={plan}
        term={open.term}
        label={label(open.term)}
        flagged={rows.kinds.some((entry) => entry.kind === 'empty' && entry.term === open.term)}
        onChange={onChange}
        onClose={close}
      />
    );
  }
  if (open.kind === 'map') {
    return (
      <MapPanel
        key={`map:${open.header}`}
        header={open.header}
        columns={rows.columns}
        label={label}
        onMap={(target) => {
          onChange((current) => withColumn(current, open.header, target));
          close();
        }}
        onClose={close}
      />
    );
  }
  const selected = open.cell;
  const cell = rows.page
    .find((entry) => entry.identifier === selected.identifier)
    ?.cells.find((entry) => entry.term === selected.term);
  if (cell === undefined) {
    return null;
  }
  return (
    <EditBar
      key={`${selected.identifier}|${selected.term}|${cell.value}`}
      identifier={selected.identifier}
      field={label(selected.term)}
      cell={cell}
      editable={editable}
      onSave={(value) => {
        onChange((current) => withEdit(current, selected.identifier, selected.term, value));
        close();
      }}
      onUndo={() => {
        onChange((current) => withoutEdit(current, selected.identifier, selected.term));
        close();
      }}
      onClose={close}
    />
  );
};

const FindBar = ({
  rows,
  view,
  shown,
  label,
  onView,
}: {
  readonly rows: RowsPreview;
  readonly view: View;
  readonly shown: number;
  readonly label: (term: string) => string;
  readonly onView: (change: Partial<View>, keepOffset?: boolean) => void;
}): ReactElement => {
  const { t } = useImportText();
  const kindText = useKindText(label);
  const filtered = rows.kinds.find((entry) => sameKind(view.filter, entry));
  return (
    <div className="import-toolbar">
      <label>
        {t('grid.find')}{' '}
        <input value={view.query} onChange={(event) => { onView({ query: event.target.value }); }} />
      </label>
      {view.filter === null ? null : (
        <span className="import-only">
          {t('grid.only', { kind: filtered === undefined ? '' : kindText(filtered) })}{' '}
          <button type="button" className="import-link" onClick={() => { onView({ filter: null }); }}>
            {t('grid.showAll')}
          </button>
        </span>
      )}
      <Pager rows={rows} view={view} shown={shown} onOffset={(offset) => { onView({ offset }, true); }} />
    </div>
  );
};

const Summary = ({
  rows,
  view,
  busy,
  onView,
  onDownload,
}: {
  readonly rows: RowsPreview;
  readonly view: View;
  readonly busy: boolean;
  readonly onView: (change: Partial<View>) => void;
  readonly onDownload: () => void;
}): ReactElement => {
  const { t, number } = useImportText();
  return (
    <div className="import-toolbar">
      <span className="query-count">
        {t('grid.summary', { total: number(rows.total), problems: number(rows.withProblems) })}
      </span>
      {rows.existing > 0 ? (
        <span className="query-count">{t('grid.summaryExisting', { count: number(rows.existing) })}</span>
      ) : null}
      <label>
        <input
          type="checkbox"
          checked={view.onlyProblems}
          onChange={(event) => { onView({ onlyProblems: event.target.checked }); }}
        />{' '}
        {t('grid.onlyProblems')}
      </label>
      {rows.existing > 0 || view.onlyExisting ? (
        <label>
          <input
            type="checkbox"
            checked={view.onlyExisting}
            onChange={(event) => { onView({ onlyExisting: event.target.checked }); }}
          />{' '}
          {t('grid.onlyExisting')}
        </label>
      ) : null}
      <button type="button" disabled={busy} onClick={onDownload}>
        {t('grid.download')}
      </button>
      {busy ? <span role="status">{t('grid.working')}</span> : null}
    </div>
  );
};

export const GridStep = ({ detail, plan, editable, onChange, problemsOnly }: TabProps): ReactElement => {
  const { t } = useImportText();
  const labels = useTermLabels();
  const [view, setView] = useState<View>({ onlyProblems: problemsOnly, onlyExisting: false, filter: null, query: '', offset: 0 });
  const [open, setOpen] = useState<Open | null>(null);
  const bulk = useBulk();
  const state = usePreview(detail.id, gridRequest(plan, view));
  const rows = state.preview?.rows;
  const label = (term: string): string =>
    labels.label(term, rows?.columns.find((entry) => entry.term === term)?.label ?? term);
  const onView = (change: Partial<View>, keepOffset = false): void => {
    setView((current) => ({ ...current, ...change, ...(keepOffset ? {} : { offset: 0 }) }));
  };
  const onFix = (fix: Fix, entry: KindEntry): void => {
    if (fix.do === 'fill') {
      setOpen(entry.term === null ? null : { kind: 'fill', term: entry.term });
    } else if (fix.do === 'sheet') {
      onChange(fix.change);
    } else {
      bulk.run(async () => {
        const found = await collectRows(detail.id, plan, selectionOf(entry));
        onChange((current) => fix.change(current, found.rows.map((row) => row.identifier)));
      });
      if (fix === SKIP) {
        onView({ filter: null });
      }
    }
  };
  const download = (): void => {
    bulk.run(async () => {
      const all = await collectRows(detail.id, plan, {});
      downloadText(`${detail.folder}.csv`, gridCsv(all.columns, all.rows.filter((row) => !row.skipped && row.existing === null)));
    });
  };
  return (
    <section className="import-grid-step">
      <p className="lede">{t('grid.lede')}</p>
      {state.error === null ? null : <ErrorNotice error={state.error} />}
      {bulk.error === null ? null : <ErrorNotice error={bulk.error} />}
      <PreviewError preview={state.preview} />
      {rows === undefined ? (
        <p role="status">{t('grid.loading')}</p>
      ) : (
        <>
          <Summary rows={rows} view={view} busy={bulk.busy} onView={onView} onDownload={download} />
          <ProblemKinds
            kinds={rows.kinds}
            plan={plan}
            filter={view.filter}
            editable={editable}
            busy={bulk.busy}
            label={label}
            onShow={(filter) => { onView({ filter }); }}
            onFix={onFix}
          />
          <FindBar rows={rows} view={view} shown={state.answered?.rows?.offset ?? 0} label={label} onView={onView} />
          <Overlays
            detail={detail}
            plan={plan}
            editable={editable}
            onChange={onChange}
            rows={rows}
            open={open}
            label={label}
            onOpen={setOpen}
          />
          {rows.page.length === 0 ? (
            <p>{t('grid.noRows')}</p>
          ) : (
            <GridTable
              rows={rows}
              selected={open?.kind === 'edit' ? open.cell : null}
              editable={editable}
              label={label}
              onPick={(cell) => { setOpen({ kind: 'edit', cell }); }}
              onFill={(term) => { setOpen({ kind: 'fill', term }); }}
              onMap={(header) => { setOpen({ kind: 'map', header }); }}
              onRowFix={(fix, identifier) => {
                if (fix.do === 'rows') {
                  onChange((current) => fix.change(current, [identifier]));
                }
              }}
              onSkip={(identifier, skip) => {
                onChange((current) => (skip ? withSkipped(current, [identifier]) : withIncluded(current, identifier)));
              }}
            />
          )}
        </>
      )}
    </section>
  );
};
