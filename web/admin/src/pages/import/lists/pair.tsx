/**
 * Pair rows and files by hand (Simple and Advanced "Pair"): rows of a list
 * that give no file of this import on one side, files no row joined on the
 * other, each sorted by name, searched and paged by the server. The person
 * picks a row, then its file; each pair is one `pair` decision. The server
 * never proposes a pair (`docs/import-v2.md`, "Unpaired lists").
 */
import { useId, useState } from 'react';
import type { ReactElement, ReactNode, SyntheticEvent } from 'react';
import type { FileRef, RowRef } from '../../../api/import-plan';
import type { Paged } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { Pager } from '../../../app/list';
import { useDraft } from '../draft-context';
import { useImportText } from '../messages';
import { fileName, folderOf } from './record-words';
import { usePreview } from './use-preview';

/** Ten a side keeps both lists in view at once. */
export const PAIR_PER_PAGE = 10;
/** Past this many rows, one at a time is slow (`docs/import-v2.md`, decision of 2026-10-20). */
export const PAIR_BY_HAND = 30;

const rowKey = (row: RowRef): string => `${row.sheet}\u0000${row.key}`;
const fileKey = (file: FileRef): string => `${file.source}\u0000${file.path}`;

interface Side {
  readonly page: number;
  readonly text: string;
}

const Search = ({ label, value, onSearch }: { readonly label: string; readonly value: string; readonly onSearch: (text: string) => void }): ReactElement => {
  const { t } = useImportText();
  const id = useId();
  const [typed, setTyped] = useState(value);
  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    onSearch(typed.trim());
  };
  return (
    <form role="search" aria-label={label} className="import-filters" onSubmit={submit}>
      <label htmlFor={id}>
        <span>{label}</span>
        <input
          id={id}
          type="search"
          value={typed}
          onChange={(event) => {
            setTyped(event.target.value);
          }}
        />
      </label>
      <button type="submit">{t('lists.search')}</button>
    </form>
  );
};

const Paging = <T,>({ data, resource, onPage }: { readonly data: Paged<T>; readonly resource: string; readonly onPage: (page: number) => void }): ReactNode => {
  const lastPage = Math.max(1, Math.ceil(data.total / data.perPage));
  return lastPage > 1 ? <Pager page={data.page} lastPage={lastPage} total={data.total} label={resource} onPage={onPage} /> : null;
};

export const PairByHand = (): ReactElement => {
  const { t, number } = useImportText();
  const draft = useDraft();
  const [rows, setRows] = useState<Side>({ page: 1, text: '' });
  const [files, setFiles] = useState<Side>({ page: 1, text: '' });
  const [picked, setPicked] = useState<RowRef | null>(null);
  const page = (side: Side) => ({ page: side.page, perPage: PAIR_PER_PAGE, ...(side.text === '' ? {} : { text: side.text }) });
  const read = usePreview(['unpaired'], { unpaired: { rows: page(rows), files: page(files) } });
  const made = draft.plan.decisions.flatMap((decision, index) => (decision.kind === 'pair' ? [{ decision, index }] : []));
  const pairedRows = new Set(made.map(({ decision }) => rowKey(decision.row)));
  const pairedFiles = new Set(made.map(({ decision }) => fileKey(decision.file)));

  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('lists.loading', { page: 1 })}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const unpaired = read.data.unpaired;
  const rowsLeft = unpaired.rows.items.filter((row) => !pairedRows.has(rowKey(row.row)));
  const filesLeft = unpaired.files.items.filter((file) => !pairedFiles.has(fileKey(file.file)));
  const pair = (file: FileRef): void => {
    if (picked === null) {
      return;
    }
    const row = picked;
    draft.change((plan) => ({ ...plan, decisions: [...plan.decisions, { kind: 'pair', row, file }] }));
    setPicked(null);
  };

  return (
    <div className="import-pair">
      <p className="lede">{t('lists.pair.lede')}</p>
      {unpaired.rows.total > PAIR_BY_HAND ? <p className="import-note">{t('lists.pair.many', { count: number(unpaired.rows.total) })}</p> : null}
      <div className="import-pair-grid" aria-busy={read.busy}>
        <section className="import-card">
          <h4>{unpaired.rows.total === 1 ? t('lists.pair.rowsOne') : t('lists.pair.rows', { count: number(unpaired.rows.total) })}</h4>
          <Search label={t('lists.pair.searchRows')} value={rows.text} onSearch={(text) => { setRows({ page: 1, text }); }} />
          {rowsLeft.length === 0 ? (
            <p className="muted">{t('lists.pair.noRows')}</p>
          ) : (
            <fieldset className="import-pick">
              <legend>{t('lists.pair.chooseRow')}</legend>
              {rowsLeft.map((row) => (
                <label key={rowKey(row.row)} className="import-opt">
                  <input
                    type="radio"
                    name="pair-row"
                    checked={picked !== null && rowKey(picked) === rowKey(row.row)}
                    disabled={!draft.editable}
                    onChange={() => {
                      setPicked(row.row);
                    }}
                  />
                  <span>
                    <b>{row.row.key}</b> {row.title === null ? null : <span className="muted">{row.title}</span>}
                  </span>
                </label>
              ))}
            </fieldset>
          )}
          <Paging data={unpaired.rows} resource={t('lists.pair.rowsResource')} onPage={(next) => { setRows({ ...rows, page: next }); }} />
        </section>
        <section className="import-card">
          <h4>{unpaired.files.total === 1 ? t('lists.pair.filesOne') : t('lists.pair.files', { count: number(unpaired.files.total) })}</h4>
          <Search label={t('lists.pair.searchFiles')} value={files.text} onSearch={(text) => { setFiles({ page: 1, text }); }} />
          {filesLeft.length === 0 ? (
            <p className="muted">{t('lists.pair.noFiles')}</p>
          ) : (
            <ul className="import-pick">
              {filesLeft.map((file) => (
                <li key={fileKey(file.file)}>
                  <span>
                    {fileName(file.file)} <span className="muted">{folderOf(file.file)}</span>
                  </span>{' '}
                  <button
                    type="button"
                    disabled={picked === null || !draft.editable}
                    onClick={() => {
                      pair(file.file);
                    }}
                  >
                    {picked === null ? t('lists.pair.pickFirst') : t('lists.pair.with', { row: picked.key })}
                  </button>
                </li>
              ))}
            </ul>
          )}
          <Paging data={unpaired.files} resource={t('lists.pair.filesResource')} onPage={(next) => { setFiles({ ...files, page: next }); }} />
        </section>
      </div>
      {made.length === 0 ? null : (
        <section>
          <h4>{t('lists.pair.made', { count: number(made.length) })}</h4>
          <ul>
            {made.slice(-10).map(({ decision, index }) => (
              <li key={index}>
                {t('lists.pair.pairOf', { row: decision.row.key, file: fileName(decision.file) })}{' '}
                <button
                  type="button"
                  className="link-button"
                  disabled={!draft.editable}
                  onClick={() => {
                    draft.change((plan) => ({ ...plan, decisions: plan.decisions.filter((_, at) => at !== index) }));
                  }}
                >
                  {t('lists.pair.undo')}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
};

export default PairByHand;
