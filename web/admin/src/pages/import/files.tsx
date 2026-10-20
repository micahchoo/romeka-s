/**
 * The Folder step's file table: what the pattern makes of each file, a page
 * at a time from the server. The filters and the files-per-item buckets are
 * new requests; "Show 50 more" asks for the next page and appends it.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { FileFilter, FileRow, FilesPreview, FilesQuery, PerItem, PreviewRequest } from '../../api/import-preview';
import { previewImport } from '../../api/imports';
import type { ImportId } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { groupTitleKey, tagClass } from './labels';
import { useImportText } from './messages';
import { requestKey } from './use-preview';
import type { PreviewState } from './use-preview';

export const FILE_PAGE = 50;

/** One item with more files than this reads as an identifier too short to tell objects apart. */
const TOO_MANY_FILES = 5;

export interface FileView {
  readonly filter: FileFilter;
  readonly perItem: PerItem | null;
}

export const filesQuery = (view: FileView, offset: number): FilesQuery =>
  view.perItem === null
    ? { filter: view.filter, offset, limit: FILE_PAGE }
    : { filter: 'match', perItem: view.perItem, offset, limit: FILE_PAGE };

const Painted = ({ row }: { readonly row: FileRow }): ReactElement => {
  const spans = Object.entries(row.groups).sort((a, b) => a[1][0] - b[1][0]);
  const out: ReactElement[] = [];
  let at = 0;
  for (const [group, [start, end]] of spans) {
    if (start < at) {
      continue;
    }
    out.push(<span key={`t${String(at)}`}>{row.path.slice(at, start)}</span>);
    out.push(
      <span key={group} className={tagClass(group)}>
        {row.path.slice(start, end)}
      </span>,
    );
    at = end;
  }
  out.push(<span key="rest">{row.path.slice(at)}</span>);
  return <>{out}</>;
};

const BUCKETS = [
  { size: 1, key: '1', text: 'files.bucket1' },
  { size: 2, key: '2', text: 'files.bucket2' },
  { size: 3, key: '3', text: 'files.bucket3' },
] as const;

const Filters = ({
  files,
  view,
  onView,
}: {
  readonly files: FilesPreview;
  readonly view: FileView;
  readonly onView: (view: FileView) => void;
}): ReactElement => {
  const { t, number } = useImportText();
  const filters = [
    { filter: 'all', text: t('files.all', { count: number(files.total) }) },
    { filter: 'miss', text: t('files.miss', { count: number(files.total - files.matched) }) },
    { filter: 'match', text: t('files.match', { count: number(files.matched) }) },
  ] as const;
  const buckets = BUCKETS.filter((bucket) => (files.buckets[bucket.key] ?? 0) > 0);
  return (
    <>
      <div className="import-chips" role="group" aria-label={t('files.show')}>
        <span>{t('files.show')}</span>
        {filters.map((entry) => (
          <button
            key={entry.filter}
            type="button"
            aria-pressed={view.perItem === null && view.filter === entry.filter}
            onClick={() => { onView({ filter: entry.filter, perItem: null }); }}
          >
            {entry.text}
          </button>
        ))}
      </div>
      {buckets.length === 0 ? null : (
        <div className="import-chips" role="group" aria-label={t('files.perItem')}>
          <span>{t('files.perItem')}</span>
          {buckets.map((bucket) => (
            <button
              key={bucket.key}
              type="button"
              aria-pressed={view.perItem === bucket.size}
              onClick={() => { onView({ filter: 'match', perItem: bucket.size }); }}
            >
              {t(bucket.text, { count: number(files.buckets[bucket.key] ?? 0) })}
            </button>
          ))}
        </div>
      )}
    </>
  );
};

const FileTableRows = ({
  rows,
  groupNames,
  onUse,
}: {
  readonly rows: readonly FileRow[];
  readonly groupNames: readonly string[];
  readonly onUse: ((path: string) => void) | null;
}): ReactElement => {
  const { t, number } = useImportText();
  return (
    <div className="table-scroll">
      <table className="import-files">
        <thead>
          <tr>
            <th>{t('files.path')}</th>
            {groupNames.map((group) => {
              const key = groupTitleKey(group);
              return (
                <th key={group}>
                  <span className={tagClass(group)}>{key === null ? group : t(key)}</span>
                </th>
              );
            })}
            <th>{t('files.item')}</th>
            <th>
              <span className="import-hidden">{t('archive.colStatus')}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.path}>
              <td className="import-mono">
                {onUse === null ? (
                  <Painted row={row} />
                ) : (
                  <button type="button" className="link-button import-path-button" title={t('example.use')} onClick={() => { onUse(row.path); }}>
                    <Painted row={row} />
                  </button>
                )}
              </td>
              {groupNames.map((group) => {
                const span = row.groups[group];
                return (
                  <td key={group} className="import-mono">
                    {span === undefined ? null : <span className={tagClass(group)}>{row.path.slice(span[0], span[1])}</span>}
                  </td>
                );
              })}
              <td>
                {row.item === null ? null : t('files.itemNumber', { number: number(row.item) })}
                {row.item !== null && row.itemFiles > 1 ? (
                  <span className="muted"> {t('files.itemFiles', { count: number(row.itemFiles) })}</span>
                ) : null}
              </td>
              <td>
                {row.matched ? (
                  <span className="badge badge-accent">{t('files.matches')}</span>
                ) : (
                  <span className="badge badge-danger">{t('files.noMatch')}</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

/** Pages after the first, kept for the request they continue. */
interface MorePages {
  readonly continues: string;
  readonly rows: readonly FileRow[];
}

export const FileTable = ({
  id,
  request,
  first,
  view,
  onView,
  onUse,
}: {
  readonly id: ImportId;
  /** The first page's request; a later page differs only in its offset. */
  readonly request: PreviewRequest & { readonly files: FilesQuery };
  readonly first: PreviewState;
  readonly view: FileView;
  readonly onView: (view: FileView) => void;
  readonly onUse: ((path: string) => void) | null;
}): ReactElement | null => {
  const { t, number } = useImportText();
  const continues = requestKey(request);
  const [more, setMore] = useState<MorePages>({ continues: '', rows: [] });
  const [loadingMore, setLoadingMore] = useState<Error | 'loading' | null>(null);
  const files = first.preview?.files;
  if (files === undefined) {
    return first.error === null ? <p role="status">{t('archive.loadingPreview')}</p> : <ErrorNotice error={first.error} />;
  }
  const extra = more.continues === continues && first.answers === continues ? more.rows : [];
  const rows = [...files.page, ...extra];
  const showMore = (): void => {
    setLoadingMore('loading');
    previewImport(api, id, { ...request, files: { ...request.files, offset: rows.length } }).then(
      (preview) => {
        setMore({ continues, rows: [...extra, ...(preview.files?.page ?? [])] });
        setLoadingMore(null);
      },
      (error: unknown) => {
        setLoadingMore(toError(error));
      },
    );
  };
  const biggest = files.biggest !== null && files.biggest.files > TOO_MANY_FILES ? files.biggest : null;
  return (
    <section className="import-file-table" aria-busy={first.loading}>
      <p className="query-count">
        {t('files.summary', { matched: number(files.matched), total: number(files.total), items: number(files.items) })}
      </p>
      {biggest === null ? null : (
        <p role="alert">{t('files.tooShort', { count: number(biggest.files), identifier: biggest.identifier })}</p>
      )}
      <Filters files={files} view={view} onView={onView} />
      <FileTableRows rows={rows} groupNames={files.groupNames} onUse={onUse} />
      {rows.length === 0 ? <p className="query-count">{t('files.empty')}</p> : null}
      {files.filtered > rows.length ? (
        <p className="query-count">
          {t('files.showing', { shown: number(rows.length), count: number(files.filtered) })}{' '}
          <button type="button" disabled={loadingMore === 'loading'} onClick={showMore}>
            {t('files.more')}
          </button>
        </p>
      ) : null}
      {loadingMore instanceof Error ? <ErrorNotice error={loadingMore} /> : null}
    </section>
  );
};
