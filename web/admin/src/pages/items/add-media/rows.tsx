/**
 * The panel's rows: a file's bytes and the media it becomes, a link being
 * created, and the files refused before they were sent. Each row states one
 * thing it is doing and offers only the actions that can change it.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import type { KeyOf, Translator } from '../../../i18n';
import { KIND_LABEL, kindOfFile, mediaKind, mediaName } from '../../../app/media';
import { MediaKindIcon } from '../../../app/media-image';
import { sizeText } from '../../../app/format';
import { currentUser } from '../../../api/session';
import type { Media } from '../../../api/types';
import type { UploadProblem } from '../../../app/upload/problem';
import type { UploadRow } from '../../../app/upload/session';
import type { AddState, AddStore, AddsSnapshot, HandOff, LinkRow, RefusedFile, SaveState } from './store';
import { mediaAdd } from '../../../i18n/namespaces/media-add';
import { resources } from '../../../i18n/namespaces/resources';

const ADMIN_ROLES: readonly string[] = ['global_admin', 'site_admin'];

const FILE_INPUT_ID = 'add-media-files';

const failedOf = (snapshot: AddsSnapshot): number =>
  [...snapshot.adds.values()].filter((state) => state.kind === 'failed').length +
  snapshot.links.filter((row) => row.state.kind === 'failed').length;

const sendingOf = (snapshot: AddsSnapshot): number =>
  snapshot.uploads.rows.filter((row) => row.phase.kind === 'waiting' || row.phase.kind === 'sending' || row.phase.kind === 'needsFile').length +
  [...snapshot.adds.values()].filter((state) => state.kind === 'adding').length +
  snapshot.links.filter((row) => row.state.kind === 'adding').length;

const addedOf = (snapshot: AddsSnapshot): number =>
  [...snapshot.adds.values()].filter((state) => state.kind === 'added').length +
  snapshot.links.filter((row) => row.state.kind === 'added').length;

type MediaAddTranslator = Translator<KeyOf<typeof mediaAdd>>;

const problemText = (problem: UploadProblem, translator: MediaAddTranslator): string => {
  const { t, locale } = translator;
  switch (problem.kind) {
    case 'fileTooLarge':
      return t('problem.fileTooLarge', { limit: sizeText(problem.limit, locale) });
    case 'importFull':
      return t('problem.importFull', { limit: sizeText(problem.limit, locale), used: sizeText(problem.used, locale) });
    case 'badName':
      return t('problem.badName');
    case 'importStarted':
      return t('problem.importStarted');
    case 'notAllowed':
      return t('problem.notAllowed');
    case 'gone':
      return t('problem.gone');
    case 'zipRefused':
      return t('problem.zipRefused', { reason: problem.reason });
    case 'connection':
      return t('problem.connection');
    case 'server':
      return t('problem.server', { message: problem.message });
  }
};

const RefusedRow = ({ file, store }: { readonly file: RefusedFile; readonly store: AddStore }): ReactElement => {
  const { t, locale } = useTranslator(mediaAdd, resources);
  const user = currentUser();
  const reason =
    file.reason.kind === 'size'
      ? t('add.refusedSize', { name: file.name, size: sizeText(file.size, locale), limit: sizeText(file.reason.limit, locale) })
      : file.reason.kind === 'extension'
        ? t('add.refusedExtension', { extension: file.reason.extension })
        : t('add.refusedNoExtension');
  return (
    <li className="add-row" data-phase="refused">
      <span className="add-row-name">{file.name}</span>
      <span className="add-row-reason" role="alert">{reason}</span>
      <span className="add-row-actions">
        {file.reason.kind === 'extension' && user !== null && ADMIN_ROLES.includes(user.role) ? (
          <a href="#/settings">{t('add.changeFiles')}</a>
        ) : null}
        <button type="button" className="link-button-danger" onClick={() => { store.removeRefused(file.key); }}>
          {t('add.remove')}
        </button>
      </span>
    </li>
  );
};

const OpenMedia = ({ media }: { readonly media: Media }): ReactElement => {
  const { t } = useTranslator(mediaAdd);
  return (
    <a className="add-row-open" href={`#/media/${String(media.id)}`}>
      {t('add.open', { id: media.id })}
    </a>
  );
};

/** A media that exists: its description (an image) or its captions (sound and moving image). */
const AddedMedia = ({
  store,
  rowKey,
  state,
  alt,
  saved,
}: {
  readonly store: AddStore;
  readonly rowKey: string;
  readonly state: AddState & { readonly kind: 'added' };
  readonly alt: string | undefined;
  readonly saved: SaveState | undefined;
}): ReactElement => {
  const { t } = useTranslator(mediaAdd, resources);
  const kind = mediaKind(state.media);
  const describe = kind === 'image';
  const helpId = `add-describe-help-${rowKey}`;
  if (!describe) {
    return kind === 'audio' || kind === 'video' ? (
      <a className="add-row-captions" href={`#/media/${String(state.media.id)}`}>{t('add.captions')}</a>
    ) : <></>;
  }
  return (
    <div className="add-describe">
      <label htmlFor={`add-describe-${rowKey}`}>{t('add.describe')}</label>
      <textarea
        id={`add-describe-${rowKey}`}
        rows={2}
        aria-describedby={helpId}
        value={alt ?? ''}
        onChange={(event) => {
          store.setAlt(rowKey, event.currentTarget.value);
        }}
        onBlur={() => {
          void store.saveAlt(rowKey);
        }}
      />
      <span className="add-hint" id={helpId}>{t('add.describeHint')}</span>
      {saved?.kind === 'saving' ? <span role="status">{t('add.adding')}</span> : null}
      {saved?.kind === 'saved' ? <span role="status">{t('add.saved')}</span> : null}
      {saved?.kind === 'failed' ? <span role="alert">{t('add.saveFailed', { reason: saved.reason })}</span> : null}
    </div>
  );
};

const UploadRowView = ({
  row,
  store,
  state,
  alt,
  saved,
}: {
  readonly row: UploadRow;
  readonly store: AddStore;
  readonly state: AddState | undefined;
  readonly alt: string | undefined;
  readonly saved: SaveState | undefined;
}): ReactElement => {
  const translator = useTranslator(mediaAdd, resources);
  const { t, locale } = translator;
  const kind = state?.kind === 'added' ? mediaKind(state.media) : kindOfFile(row.path, null);
  const name = state?.kind === 'added' ? (mediaName(state.media) ?? row.path) : row.path;
  const size = sizeText(row.size, locale);
  const progress = row.phase.kind === 'sending' || row.phase.kind === 'paused' || row.phase.kind === 'waiting';
  const phase = state?.kind === 'added' ? 'added' : state?.kind === 'adding' ? 'adding' : row.phase.kind;
  const uploadProblem = row.phase.kind === 'failed' ? row.phase.problem : null;
  return (
    <li className="add-row" data-phase={phase}>
      <span className="add-row-kind">
        <MediaKindIcon kind={kind} />
        <span>{t(KIND_LABEL[kind])}</span>
      </span>
      <span className="add-row-name">{name}</span>
      <span className="add-row-facts">
        {state?.kind === 'added'
          ? t('add.kindSize', { kind: t(KIND_LABEL[kind]), size })
          : t('add.expected', { kind: t(KIND_LABEL[kind]), size })}
      </span>
      {progress ? (
        <>
          <progress max={Math.max(row.size, 1)} value={row.sent} aria-label={t('add.progressLabel', { name: row.path })} />
          <span className="add-row-state">
            {row.phase.kind === 'waiting'
              ? t('add.waiting')
              : row.phase.kind === 'paused'
                ? t('add.paused')
                : t('add.sending', { sent: sizeText(row.sent, locale), size })}
          </span>
        </>
      ) : null}
      {row.phase.kind === 'needsFile' ? (
        <>
          <span className="add-row-reason">{t('add.needsFile', { sent: sizeText(row.sent, locale), size, name: row.path })}</span>
          <span className="add-row-actions">
            <label className="button" htmlFor={FILE_INPUT_ID}>{t('add.chooseAgain')}</label>
            <button type="button" className="link-button-danger" onClick={() => { store.discard(row.key); }}>{t('add.discard')}</button>
          </span>
        </>
      ) : null}
      {state?.kind === 'adding' ? <span className="add-row-state">{t('add.adding')}</span> : null}
      {state?.kind === 'added' ? (
        <>
          <span className="add-row-state">{t('add.added')}</span>
          <OpenMedia media={state.media} />
          <AddedMedia store={store} rowKey={row.key} state={state} alt={alt} saved={saved} />
        </>
      ) : null}
      {state?.kind === 'failed' ? (
        <>
          <span className="add-row-reason" role="alert">{state.reason}</span>
          <span className="add-row-actions">
            <button type="button" onClick={() => { store.retry(row.key); }}>{t('add.tryAgain')}</button>
            <button type="button" className="link-button-danger" onClick={() => { store.discard(row.key); }}>{t('add.remove')}</button>
          </span>
        </>
      ) : null}
      {uploadProblem === null ? null : (
        <>
          <span className="add-row-reason" role="alert">{problemText(uploadProblem, translator)}</span>
          <span className="add-row-actions">
            <button type="button" onClick={() => { store.retry(row.key); }}>{t('add.tryAgain')}</button>
            <button type="button" className="link-button-danger" onClick={() => { store.discard(row.key); }}>{t('add.remove')}</button>
          </span>
        </>
      )}
      {row.phase.kind === 'sending' ? (
        <span className="add-row-actions">
          <button type="button" onClick={() => { store.pause(row.key); }}>{t('add.pause')}</button>
          <button type="button" className="link-button-danger" onClick={() => { store.discard(row.key); }}>{t('add.remove')}</button>
        </span>
      ) : null}
      {row.phase.kind === 'paused' ? (
        <span className="add-row-actions">
          <button type="button" onClick={() => { store.resume(row.key); }}>{t('add.resume')}</button>
          <button type="button" className="link-button-danger" onClick={() => { store.discard(row.key); }}>{t('add.remove')}</button>
        </span>
      ) : null}
    </li>
  );
};

const LinkRowView = ({ row, store }: { readonly row: LinkRow; readonly store: AddStore }): ReactElement => {
  const { t } = useTranslator(mediaAdd, resources);
  return (
    <li className="add-row" data-phase={row.state.kind}>
      <span className="add-row-name">{row.label}</span>
      {row.state.kind === 'adding' ? <span className="add-row-state">{t('add.adding')}</span> : null}
      {row.state.kind === 'added' ? (
        <>
          <span className="add-row-state">{t('add.added')}</span>
          <OpenMedia media={row.state.media} />
        </>
      ) : null}
      {row.state.kind === 'failed' ? (
        <>
          <span className="add-row-reason" role="alert">{row.state.reason}</span>
          <span className="add-row-actions">
            <button type="button" onClick={() => { store.retryLink(row.key); }}>{t('add.tryAgain')}</button>
            <button type="button" className="link-button-danger" onClick={() => { store.discard(row.key); }}>{t('add.remove')}</button>
          </span>
        </>
      ) : null}
    </li>
  );
};

const HandOffNote = ({ handOff }: { readonly handOff: HandOff }): ReactElement => {
  const { t } = useTranslator(mediaAdd);
  const text =
    handOff.kind === 'many'
      ? t('add.handOffMany', { count: handOff.count })
      : handOff.kind === 'folder'
        ? t('add.handOffFolder')
        : t('add.handOffZip');
  return (
    <p className="add-hand-off" role="alert">
      {text} <a href="#/import">{t('add.importLink')}</a>
    </p>
  );
};

export const AddRows = ({ store, snapshot }: { readonly store: AddStore; readonly snapshot: AddsSnapshot }): ReactElement | null => {
  const { t, number } = useTranslator(mediaAdd);
  const rows = snapshot.uploads.rows;
  const added = addedOf(snapshot);
  const sending = sendingOf(snapshot);
  const failed = failedOf(snapshot);
  const total = rows.length + snapshot.links.length + snapshot.refused.length;
  if (total === 0 && snapshot.handOff === null) {
    return null;
  }
  return (
    <div className="add-rows">
      {snapshot.handOff === null ? null : <HandOffNote handOff={snapshot.handOff} />}
      {added + sending + failed === 0 ? null : (
        <p className="add-summary" role="status" aria-live="polite">
          {added > 0 && sending === 0 && failed === 0
            ? t('add.summaryDone', { added: number(added) })
            : t('add.summary', { added: number(added), sending: number(sending), failed: number(failed) })}
        </p>
      )}
      <ul className="add-list" aria-label={t('add.listLabel')}>
        {snapshot.refused.map((file) => (
          <RefusedRow key={file.key} file={file} store={store} />
        ))}
        {snapshot.links.map((row) => (
          <LinkRowView key={row.key} row={row} store={store} />
        ))}
        {rows.map((row) => (
          <UploadRowView
            key={row.key}
            row={row}
            store={store}
            state={snapshot.adds.get(row.key)}
            alt={snapshot.alt.get(row.key)}
            saved={snapshot.saved.get(row.key)}
          />
        ))}
      </ul>
    </div>
  );
};
