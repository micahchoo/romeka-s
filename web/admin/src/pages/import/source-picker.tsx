/**
 * "Where are your files?": a folder under one of the server's archive roots
 * (`GET /api/romeka/archive`), or this computer. The folder button names the
 * folder it starts a draft over, and the draft opens on "What we found"; the
 * computer button starts a draft over the import's own staging area, and it
 * opens on "Your files", where the files are added.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { ArchiveListing } from '../../api/import-view';
import { importApi } from '../../api/import-routes';
import { ErrorNotice } from '../../app/errors';
import { Pager } from '../../app/list';
import { toError, useAsync } from '../../app/useAsync';
import { useImportText } from './messages';
import { PER_PAGE } from './paged-list';
import { placeHref } from './screens';

interface At {
  /** `undefined` until the person picks one: the server lists its first root. */
  readonly root: string | undefined;
  readonly path: string;
}

const parentPaths = (path: string): readonly string[] => {
  const parts = path === '' ? [] : path.split('/');
  return parts.map((_, index) => parts.slice(0, index + 1).join('/'));
};

const nameOf = (path: string): string => path.split('/').at(-1) ?? path;

const Trail = ({ listing, onOpen }: { readonly listing: ArchiveListing; readonly onOpen: (path: string) => void }): ReactElement => {
  const { t } = useImportText();
  const root = listing.root ?? '';
  const trail = ['', ...parentPaths(listing.path)];
  return (
    <nav aria-label={t('home.server.path')} className="import-trail">
      <ol>
        {trail.map((path) => (
          <li key={path}>
            {path === listing.path ? (
              <span aria-current="location">{path === '' ? root : nameOf(path)}</span>
            ) : (
              <button type="button" className="link-button" onClick={() => { onOpen(path); }}>
                {path === '' ? root : nameOf(path)}
              </button>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
};

const Folders = ({ listing, onOpen }: { readonly listing: ArchiveListing; readonly onOpen: (path: string) => void }): ReactElement => {
  const { t } = useImportText();
  const [page, setPage] = useState(1);
  const folders = listing.entries.filter((entry) => entry.kind === 'folder');
  if (folders.length === 0) {
    return <p>{t('home.server.noFolders')}</p>;
  }
  const lastPage = Math.ceil(folders.length / PER_PAGE);
  return (
    <>
      <ul className="import-folders">
        {folders.slice((page - 1) * PER_PAGE, page * PER_PAGE).map((entry) => (
          <li key={entry.path}>
            <button type="button" onClick={() => { onOpen(entry.path); }}>
              {entry.name}
            </button>
          </li>
        ))}
      </ul>
      {lastPage > 1 ? (
        <Pager page={page} lastPage={lastPage} total={folders.length} label={t('home.server.folders')} onPage={setPage} />
      ) : null}
    </>
  );
};

const UseFolder = ({ listing }: { readonly listing: ArchiveListing }): ReactElement | null => {
  const { t } = useImportText();
  const [state, setState] = useState<{ readonly busy: boolean; readonly error: Error | null }>({ busy: false, error: null });
  const root = listing.root;
  if (root === null) {
    return null;
  }
  const start = (): void => {
    setState({ busy: true, error: null });
    importApi.create({ sources: [{ id: 's1', kind: 'folder', root, path: listing.path }] }).then(
      (created) => {
        window.location.hash = placeHref(created.id, { mode: 'simple', screen: 'structure' });
      },
      (error: unknown) => {
        setState({ busy: false, error: toError(error) });
      },
    );
  };
  const files = listing.entries.filter((entry) => entry.kind === 'file').length;
  return (
    <>
      <p className="muted">
        {files === 1 ? t('home.server.filesOne') : t('home.server.files', { count: files })}
      </p>
      <p>
        <button type="button" className="import-primary" disabled={state.busy} onClick={start}>
          {state.busy
            ? t('home.server.starting')
            : listing.path === ''
              ? t('home.server.useAll', { root })
              : t('home.server.use', { folder: listing.path })}
        </button>
      </p>
      {state.error === null ? null : <ErrorNotice error={state.error} />}
    </>
  );
};

const ComputerStart = (): ReactElement => {
  const { t } = useImportText();
  const [state, setState] = useState<{ readonly busy: boolean; readonly error: Error | null }>({ busy: false, error: null });
  const start = (): void => {
    setState({ busy: true, error: null });
    importApi.create({ sources: [{ id: 's1', kind: 'staging' }] }).then(
      (created) => {
        window.location.hash = placeHref(created.id, { mode: 'simple', screen: 'files' });
      },
      (error: unknown) => {
        setState({ busy: false, error: toError(error) });
      },
    );
  };
  return (
    <>
      <p>
        <button type="button" className="import-primary" disabled={state.busy} onClick={start}>
          {state.busy ? t('home.server.starting') : t('home.computer.start')}
        </button>
      </p>
      {state.error === null ? null : <ErrorNotice error={state.error} />}
    </>
  );
};

const ServerFolders = (): ReactElement => {
  const { t } = useImportText();
  const [at, setAt] = useState<At>({ root: undefined, path: '' });
  const listing = useAsync(() => importApi.archive(at.root === undefined ? { path: at.path } : { root: at.root, path: at.path }), [at.root, at.path]);
  if (listing.status === 'loading') {
    return <p role="status" className="import-slow">{t('home.server.loading')}</p>;
  }
  if (listing.status === 'error') {
    return (
      <>
        <ErrorNotice error={listing.error} />
        {at.path === '' ? null : (
          <button type="button" onClick={() => { setAt({ root: at.root, path: '' }); }}>
            {t('home.server.top')}
          </button>
        )}
      </>
    );
  }
  const data = listing.data;
  if (data.root === null) {
    return <p>{t('home.server.none')}</p>;
  }
  const root = data.root;
  const open = (path: string): void => {
    setAt({ root, path });
  };
  return (
    <>
      {data.roots.length > 1 ? (
        <div role="group" aria-label={t('home.server.roots')} className="import-roots">
          {data.roots.map((name) => (
            <button key={name} type="button" aria-pressed={name === root} onClick={() => { setAt({ root: name, path: '' }); }}>
              {name}
            </button>
          ))}
        </div>
      ) : null}
      <Trail listing={data} onOpen={open} />
      <Folders key={`${root}/${data.path}`} listing={data} onOpen={open} />
      <UseFolder key={`use:${root}/${data.path}`} listing={data} />
    </>
  );
};

export const SourcePicker = (): ReactElement => {
  const { t } = useImportText();
  return (
    <section className="import-sources">
      <h2>{t('home.where')}</h2>
      <div className="import-cards">
        <section className="import-card">
          <h3>{t('home.server.title')}</h3>
          <p className="muted">{t('home.server.note')}</p>
          <ServerFolders />
        </section>
        <section className="import-card">
          <h3>{t('home.computer.title')}</h3>
          <p className="muted">{t('home.computer.note')}</p>
          <ComputerStart />
        </section>
      </div>
    </section>
  );
};
