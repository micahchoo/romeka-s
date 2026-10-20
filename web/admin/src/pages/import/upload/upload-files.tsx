/**
 * Add files to an import from this computer: drop files, a folder or a zip,
 * or choose them with a button. Sending, resuming and the error words live in
 * `session.ts` and `text.ts`; this view renders a snapshot and forwards
 * presses.
 */
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import type { DragEvent, ReactElement } from 'react';
import type { ImportId } from '../../../api/import-plan';
import { uploadApi } from '../../../api/import-uploads';
import type { UploadApi } from '../../../api/import-uploads';
import { useImportText } from '../messages';
import type { ImportTranslator } from '../messages';
import { fromBrowserEntry, fromDrop, fromInput } from './pick';
import type { Picked } from './pick';
import { UploadSession, totals } from './session';
import type { UploadRow, UploadSnapshot } from './session';
import { problemText, sizeText, stateText } from './text';
import './upload.css';

export interface UploadFilesProps {
  readonly importId: ImportId;
  /** The files on the server changed (one arrived whole, a zip was unpacked, one was removed): look at the files again. */
  readonly onFilesChanged?: () => void;
  /** Every change of what the view shows, for a shell that holds Next while files are still on their way. */
  readonly onChange?: (snapshot: UploadSnapshot) => void;
  /** For tests: the client, and the delay between tries and job polls. */
  readonly api?: UploadApi;
  readonly wait?: (ms: number) => Promise<void>;
}

/** Rows shown in each list; the rest are counted, so a dropped folder of 10,000 files stays light. */
const SHOWN = 200;
const RETRYABLE = new Set(['connection', 'server', 'gone', 'importFull']);

const useSession = (props: UploadFilesProps): UploadSession | null => {
  const { importId, api = uploadApi, wait } = props;
  const changed = useRef(props.onFilesChanged);
  changed.current = props.onFilesChanged;
  const [session, setSession] = useState<UploadSession | null>(null);
  useEffect(() => {
    const created = new UploadSession({
      api,
      importId,
      ...(wait === undefined ? {} : { wait }),
      onFilesChanged: () => changed.current?.(),
    });
    setSession(created);
    void created.load();
    return () => {
      created.dispose();
    };
  }, [api, importId, wait]);
  return session;
};

const EMPTY_SUBSCRIBE = (): (() => void) => () => undefined;

const useSnapshot = (session: UploadSession | null): UploadSnapshot | null =>
  useSyncExternalStore(session?.subscribe ?? EMPTY_SUBSCRIBE, session?.snapshot ?? (() => null));

/** One polite sentence per file that finished or failed, never per byte. */
const useAnnouncement = (snapshot: UploadSnapshot | null, translator: ImportTranslator): string => {
  const seen = useRef(new Map<string, UploadRow['phase']['kind']>());
  const [said, setSaid] = useState('');
  useEffect(() => {
    if (snapshot === null) {
      return;
    }
    const { t } = translator;
    const sum = totals(snapshot.rows);
    let next: string | null = null;
    for (const row of snapshot.rows) {
      const before = seen.current.get(row.key);
      if (before !== undefined && before !== row.phase.kind) {
        if (row.phase.kind === 'done') {
          next = t('upload.announce.finished', { name: row.path, done: sum.done, count: sum.count });
        } else if (row.phase.kind === 'failed') {
          next = t('upload.announce.failed', { name: row.path, reason: problemText(row.phase.problem, translator) });
        }
      }
    }
    if (next !== null && sum.count > 0 && sum.done === sum.count) {
      next = t('upload.announce.allDone');
    }
    seen.current = new Map(snapshot.rows.map((row) => [row.key, row.phase.kind]));
    if (next !== null) {
      setSaid(next);
    }
  }, [snapshot, translator]);
  return said;
};

/** The browser asks before the page closes while bytes are still on their way. */
const useLeaveGuard = (busy: boolean): void => {
  useEffect(() => {
    if (!busy) {
      return;
    }
    const ask = (event: BeforeUnloadEvent): void => {
      event.preventDefault();
    };
    window.addEventListener('beforeunload', ask);
    return () => {
      window.removeEventListener('beforeunload', ask);
    };
  }, [busy]);
};

const FileRow = ({ row, session, translator, closed }: {
  readonly row: UploadRow;
  readonly session: UploadSession;
  readonly translator: ImportTranslator;
  readonly closed: boolean;
}): ReactElement => {
  const { t, locale } = translator;
  const { kind } = row.phase;
  const failed = row.phase.kind === 'failed';
  const retryable = row.phase.kind === 'failed' && !closed && RETRYABLE.has(row.phase.problem.kind);
  return (
    <li className="upload-row" data-phase={kind}>
      <span className="upload-path">{row.path}</span>
      {kind === 'done' ? null : (
        <progress max={Math.max(row.size, 1)} value={row.sent} aria-label={row.path} />
      )}
      <span className={failed ? 'upload-state upload-state-failed' : 'upload-state'}>
        {stateText(row, translator)}
        {kind === 'sending' || kind === 'paused' || kind === 'waiting' ? (
          <> {t('upload.progress', { sent: sizeText(row.sent, locale), size: sizeText(row.size, locale) })}</>
        ) : null}
        {kind === 'done' ? <> {sizeText(row.size, locale)}</> : null}
      </span>
      <span className="upload-actions">
        {kind === 'sending' || kind === 'waiting' ? (
          <button type="button" aria-label={t('upload.pauseFile', { name: row.path })} onClick={() => { session.pause(row.key); }}>
            {t('upload.pause')}
          </button>
        ) : null}
        {kind === 'paused' ? (
          <button type="button" aria-label={t('upload.resumeFile', { name: row.path })} onClick={() => { session.resume(row.key); }}>
            {t('upload.resume')}
          </button>
        ) : null}
        {retryable ? (
          <button type="button" aria-label={t('upload.retryFile', { name: row.path })} onClick={() => { session.retry(row.key); }}>
            {t('upload.retry')}
          </button>
        ) : null}
        {kind === 'unpacking' ? null : (
          <button type="button" className="link-button-danger" aria-label={t('upload.removeFile', { name: row.path })} onClick={() => { void session.remove(row.key); }}>
            {t('upload.remove')}
          </button>
        )}
      </span>
    </li>
  );
};

const RowList = ({ rows, label, ...rest }: {
  readonly rows: readonly UploadRow[];
  readonly label: string;
  readonly session: UploadSession;
  readonly translator: ImportTranslator;
  readonly closed: boolean;
}): ReactElement => (
  <>
    <ul className="upload-rows" aria-label={label}>
      {rows.slice(0, SHOWN).map((row) => (
        <FileRow key={row.key} row={row} {...rest} />
      ))}
    </ul>
    {rows.length > SHOWN ? <p className="upload-more">{rest.translator.t('upload.moreRows', { count: rest.translator.number(rows.length - SHOWN) })}</p> : null}
  </>
);

const DropZone = ({ session, translator }: { readonly session: UploadSession; readonly translator: ImportTranslator }): ReactElement => {
  const { t } = translator;
  const [over, setOver] = useState(false);
  const [reading, setReading] = useState(false);
  const depth = useRef(0);
  const filesInput = useRef<HTMLInputElement>(null);
  const folderInput = useRef<HTMLInputElement | null>(null);
  // React has no `webkitdirectory` prop; the folder picker needs the DOM property.
  const setFolderInput = useCallback((input: HTMLInputElement | null) => {
    folderInput.current = input;
    if (input !== null) {
      input.webkitdirectory = true;
    }
  }, []);
  const add = (picked: Picked): void => {
    void session.add(picked);
  };
  const onDrop = (event: DragEvent<HTMLDivElement>): void => {
    event.preventDefault();
    depth.current = 0;
    setOver(false);
    // The items are readable only inside this handler.
    const entries = Array.from(event.dataTransfer.items, (item) => (item.kind === 'file' ? fromBrowserEntry(item.webkitGetAsEntry()) : null));
    const files = Array.from(event.dataTransfer.files);
    setReading(true);
    void fromDrop(entries, files).then(add).finally(() => { setReading(false); });
  };
  const picked = (input: HTMLInputElement): void => {
    if (input.files !== null) {
      add(fromInput(input.files));
    }
    input.value = '';
  };
  return (
    <div
      className="upload-drop"
      data-over={over}
      onDragEnter={(event) => { event.preventDefault(); depth.current += 1; setOver(true); }}
      onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = 'copy'; }}
      onDragLeave={() => { depth.current -= 1; if (depth.current <= 0) { depth.current = 0; setOver(false); } }}
      onDrop={onDrop}
    >
      <p className="upload-drop-hint">{reading ? t('upload.reading') : over ? t('upload.dropNow') : t('upload.dropHint')}</p>
      <div className="upload-pick">
        <button type="button" onClick={() => filesInput.current?.click()}>{t('upload.chooseFiles')}</button>
        <button type="button" onClick={() => folderInput.current?.click()}>{t('upload.chooseFolder')}</button>
      </div>
      <p className="upload-note">{t('upload.zipNote')}</p>
      <input ref={filesInput} type="file" multiple hidden tabIndex={-1} onChange={(event) => { picked(event.currentTarget); }} />
      <input
        ref={setFolderInput}
        type="file"
        hidden
        tabIndex={-1}
        onChange={(event) => { picked(event.currentTarget); }}
      />
    </div>
  );
};

export const UploadFiles = (props: UploadFilesProps): ReactElement => {
  const translator = useImportText();
  const { t, locale, number } = translator;
  const session = useSession(props);
  const snapshot = useSnapshot(session);
  const said = useAnnouncement(snapshot, translator);
  const onChange = props.onChange;
  useEffect(() => {
    if (snapshot !== null) {
      onChange?.(snapshot);
    }
  }, [snapshot, onChange]);
  const rows = snapshot?.rows ?? [];
  const moving = rows.some((row) => row.phase.kind === 'sending' || row.phase.kind === 'waiting');
  useLeaveGuard(moving);
  const sum = totals(rows);
  const closed = snapshot?.closed ?? null;
  const open = rows.filter((row) => row.phase.kind !== 'done');
  const done = rows.filter((row) => row.phase.kind === 'done');
  const paused = rows.some((row) => row.phase.kind === 'paused');
  return (
    <section className="upload" aria-labelledby="upload-heading">
      <h3 id="upload-heading">{t('upload.heading')}</h3>
      {session === null || snapshot === null ? null : (
        <>
          {closed === null ? <DropZone session={session} translator={translator} /> : <p role="alert">{problemText(closed, translator)}</p>}
          {snapshot.loaded ? null : <p className="upload-note">{t('upload.loading')}</p>}
          {snapshot.loadProblem !== null && closed === null ? <p role="alert">{problemText(snapshot.loadProblem, translator)}</p> : null}
          {snapshot.limit === null ? null : (
            <p className="upload-note">
              {t('upload.limits', { file: sizeText(snapshot.limit.file, locale), total: sizeText(snapshot.limit.total, locale), used: sizeText(snapshot.used, locale) })}
            </p>
          )}
          {snapshot.stored.count > 0 ? <p>{t('upload.stored', { count: number(snapshot.stored.count), size: sizeText(snapshot.stored.bytes, locale) })}</p> : null}
          {snapshot.skipped > 0 ? <p className="upload-note">{t('upload.skipped', { count: number(snapshot.skipped) })}</p> : null}
          {snapshot.alreadyHere > 0 ? <p className="upload-note">{t('upload.alreadyHere', { count: number(snapshot.alreadyHere) })}</p> : null}
          {sum.count > 0 ? (
            <div className="upload-overall">
              <progress max={Math.max(sum.size, 1)} value={sum.sent} aria-label={t('upload.overallLabel')} />
              <p>
                {t('upload.overall', { sent: sizeText(sum.sent, locale), size: sizeText(sum.size, locale), done: number(sum.done), count: number(sum.count) })}
              </p>
              <div className="upload-pick">
                {moving ? <button type="button" onClick={() => { session.pauseAll(); }}>{t('upload.pauseAll')}</button> : null}
                {paused ? <button type="button" onClick={() => { session.resumeAll(); }}>{t('upload.resumeAll')}</button> : null}
              </div>
              {moving ? <p className="upload-note">{t('upload.leaveNote')}</p> : null}
            </div>
          ) : null}
          {open.length > 0 ? <RowList rows={open} label={t('upload.listLabel')} session={session} translator={translator} closed={closed !== null} /> : null}
          {done.length > 0 ? (
            <details className="upload-done">
              <summary>{t('upload.doneList', { count: number(done.length) })}</summary>
              <RowList rows={done} label={t('upload.doneList', { count: number(done.length) })} session={session} translator={translator} closed={closed !== null} />
            </details>
          ) : null}
        </>
      )}
      <p className="upload-live" role="status" aria-live="polite">{said}</p>
    </section>
  );
};
