/**
 * `#/import`: start an import by choosing a folder under the archive root,
 * and the history of earlier imports, newest first.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { createImport, getArchive, listImports } from '../../api/imports';
import type { ArchiveListing, ImportRecord } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { formatDateTime } from '../../app/format';
import { toError, useAsync } from '../../app/useAsync';
import { useImportText } from './messages';
import { tabHref } from './tabs';
import { UndoNotice } from './undo';

const parentOf = (path: string): string => path.split('/').slice(0, -1).join('/');
const childOf = (path: string, name: string): string => (path === '' ? name : `${path}/${name}`);

const FolderList = ({
  listing,
  onOpen,
}: {
  readonly listing: ArchiveListing;
  readonly onOpen: (path: string) => void;
}): ReactElement => {
  const { t, number } = useImportText();
  const folders = listing.entries.filter((entry) => entry.kind === 'dir');
  return (
    <>
      <p className="import-path">
        {t('archive.root')}: <code>/{listing.path}</code>{' '}
        {listing.path === '' ? null : (
          <button type="button" className="link-button" onClick={() => { onOpen(parentOf(listing.path)); }}>
            {t('archive.up')}
          </button>
        )}
      </p>
      {folders.length === 0 ? (
        <p>{t('archive.noFolders')}</p>
      ) : (
        <ul className="import-chips import-folders">
          {folders.map((entry) => (
            <li key={entry.name}>
              <button type="button" onClick={() => { onOpen(childOf(listing.path, entry.name)); }}>
                <span className="import-mono">{entry.name}/</span>
                {entry.files === undefined ? null : (
                  <small>{t('archive.folderFiles', { count: number(entry.files) })}</small>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
};

const StartImport = (): ReactElement => {
  const { t } = useImportText();
  const [path, setPath] = useState('');
  const [creating, setCreating] = useState<{ readonly busy: boolean; readonly error: Error | null }>({
    busy: false,
    error: null,
  });
  const state = useAsync(() => getArchive(api, path), [path]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.page')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  const listing = state.data;
  if (!listing.enabled) {
    return <p role="alert">{t('archive.disabled')}</p>;
  }
  const start = (): void => {
    setCreating({ busy: true, error: null });
    createImport(api, listing.path).then(
      (created) => {
        window.location.hash = tabHref(created.id, 'folder');
      },
      (error: unknown) => {
        setCreating({ busy: false, error: toError(error) });
      },
    );
  };
  return (
    <section className="import-start">
      <h2>{t('archive.start')}</h2>
      <FolderList listing={listing} onOpen={setPath} />
      {listing.path === '' ? null : (
        <p>
          <button type="button" className="button-primary" disabled={creating.busy} onClick={start}>
            {creating.busy ? t('archive.creating') : t('archive.importThis', { folder: listing.path })}
          </button>
        </p>
      )}
      {creating.error === null ? null : <ErrorNotice error={creating.error} />}
    </section>
  );
};

const History = ({ rows }: { readonly rows: readonly ImportRecord[] }): ReactElement => {
  const { t, number, locale } = useImportText();
  return (
    <section className="import-history">
      <h2>{t('archive.history')}</h2>
      {rows.length === 0 ? (
        <p>{t('archive.historyEmpty')}</p>
      ) : (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>{t('archive.colFolder')}</th>
                <th>{t('archive.colStatus')}</th>
                <th>{t('archive.colCreated')}</th>
                <th>{t('archive.colFinished')}</th>
                <th className="cell-num">{t('archive.colItems')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id}>
                  <td>
                    <a href={tabHref(row.id, row.status === 'draft' ? 'folder' : 'run')}>{row.folder}</a>
                  </td>
                  <td>{t(`archive.status.${row.status}`)}</td>
                  <td>{formatDateTime(row.created, locale)}</td>
                  <td>{row.finished === null ? '' : formatDateTime(row.finished, locale)}</td>
                  <td className="cell-num">{row.summary === null ? '' : number(row.summary.items)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
};

/** One read of the history feeds the undo banner and the table. */
export const ImportHome = (): ReactElement => {
  const { t } = useImportText();
  const history = useAsync(() => listImports(api), []);
  return (
    <>
      {history.status === 'ready' ? <UndoNotice imports={history.data} /> : null}
      <p className="lede">{t('archive.lede')}</p>
      <StartImport />
      {history.status === 'loading' ? <p role="status">{t('loading.page')}</p> : null}
      {history.status === 'error' ? <ErrorNotice error={history.error} /> : null}
      {history.status === 'ready' ? <History rows={history.data} /> : null}
    </>
  );
};
