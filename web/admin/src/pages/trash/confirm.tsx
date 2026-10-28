/**
 * The delete of one item, media or item set from its own page. The record moves to the trash as
 * a bulk delete of it alone, so Undo puts it back: an item with its media, its files and the
 * links other records had to it; a media on its item with its file; a set with its items and the
 * sites that show it. The confirm states only what the server counts: the media and their bytes
 * (the delete preview), the records that link to it by field (`/api/romeka/links/{id}`), and how
 * many days the trash keeps it. A set's page adds its own counts as `detail`.
 *
 * ```text
 * checking -> confirm -> moving -> (its list screen, which offers the Undo)
 * checking -> unchecked -(check again)-> checking
 * moving -> failed -(check again)-> checking
 * ```
 *
 * Its own chunk, loaded when the person presses Delete.
 */
import type { ReactElement, ReactNode } from 'react';
import { useCallback, useEffect, useReducer, useRef } from 'react';
import type { BulkRefusal, BulkRun } from '../../api/bulk';
import { bulkApi, bulkRefusal } from '../../api/bulk';
import type { BulkDeletePreview } from '../../api/bulk-delete';
import { bulkDeleteApi } from '../../api/bulk-delete';
import { placesApi } from '../../api/places';
import type { LinkRow } from '../../api/places';
import { ErrorNotice } from '../../app/errors';
import { sizeText } from '../../app/upload/size-text';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { bulkDelete } from '../../i18n/namespaces/bulk-delete';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useLinkRowWords } from '../items/linked-from';
import { ServerWords } from '../items/server-words';
import { TRASH_KINDS } from './kinds';
import { rememberTrashed } from './slot';
import type { TrashTarget } from './slot';

interface Facts {
  readonly preview: BulkDeletePreview;
  readonly links: readonly LinkRow[];
}

type State =
  | { readonly step: 'checking' }
  | { readonly step: 'unchecked'; readonly refusal: BulkRefusal }
  | { readonly step: 'confirm'; readonly facts: Facts }
  | { readonly step: 'moving'; readonly facts: Facts }
  | { readonly step: 'failed'; readonly facts: Facts; readonly run: BulkRun | null; readonly error: Error | null };

type Action =
  | { readonly type: 'check' }
  | { readonly type: 'checked'; readonly facts: Facts }
  | { readonly type: 'uncheckable'; readonly refusal: BulkRefusal }
  | { readonly type: 'move' }
  | { readonly type: 'failed'; readonly run: BulkRun | null; readonly error: Error | null };

const reduce = (state: State, action: Action): State => {
  switch (action.type) {
    case 'check':
      return { step: 'checking' };
    case 'checked':
      return state.step === 'checking' ? { step: 'confirm', facts: action.facts } : state;
    case 'uncheckable':
      return state.step === 'checking' ? { step: 'unchecked', refusal: action.refusal } : state;
    case 'move':
      return state.step === 'confirm' ? { step: 'moving', facts: state.facts } : state;
    case 'failed':
      return state.step === 'moving' ? { step: 'failed', facts: state.facts, run: action.run, error: action.error } : state;
  }
};

/**
 * Whether the preview found the record and the caller may delete it. A missing record and one
 * the caller may not see both select nothing, so they read alike.
 */
const movable = (preview: BulkDeletePreview): boolean => preview.total === 1 && preview.denied === 0;

/** Whether the run moved this one record to the trash. */
const trashed = (run: BulkRun): boolean => run.status === 'done' && run.counts?.done === 1;

/** What goes with the record's files: an item's media, or a media's own file. */
const Files = ({ target, preview }: { readonly target: TrashTarget; readonly preview: BulkDeletePreview }): ReactElement | null => {
  const { t, number, locale } = useTranslator(bulkDelete);
  const size = sizeText(preview.bytes, locale);
  if (target.of === 'media') {
    return preview.bytes > 0 ? <p>{t('bulkDelete.media.file', { size })}</p> : null;
  }
  if (preview.media === 1) {
    return <p>{t('bulkDelete.confirm.mediaOne', { size })}</p>;
  }
  return preview.media > 1 ? <p>{t('bulkDelete.confirm.mediaMany', { count: number(preview.media), size })}</p> : null;
};

const Facts = ({
  target,
  name,
  facts,
  detail,
}: {
  readonly target: TrashTarget;
  readonly name: string;
  readonly facts: Facts;
  readonly detail: ReactNode;
}): ReactElement => {
  const { t, number } = useTranslator(bulkDelete);
  const rowWords = useLinkRowWords();
  const { preview, links } = facts;
  return (
    <>
      <p>{t('bulkDelete.item.ask', { name })}</p>
      <Files target={target} preview={preview} />
      {detail}
      {links.length === 0 ? (
        <p>{t('bulkDelete.item.notLinked')}</p>
      ) : (
        <>
          <p>{t('bulkDelete.item.linked')}</p>
          <ul>
            {links.map((row) => (
              <li key={`${row.kind}:${String(row.propertyId)}`}>{rowWords(row)}</li>
            ))}
          </ul>
          <p>{t('bulkDelete.item.linksBack')}</p>
        </>
      )}
      <p>{t(TRASH_KINDS[target.of].trash, { days: number(preview.trashDays) })}</p>
    </>
  );
};

/** Why the run left the record in place: its error, or the server's reason for the record. */
const Failure = ({ run, error }: { readonly run: BulkRun | null; readonly error: Error | null }): ReactElement | null => {
  if (error !== null) {
    return <ErrorNotice error={error} />;
  }
  const reasons = [run?.error ?? null, ...(run?.failures ?? []).map((failure) => failure.message)].filter(
    (message): message is string => message !== null,
  );
  return reasons.length === 0 ? null : (
    <div role="alert">
      <ServerWords messages={reasons} />
    </div>
  );
};

export interface TrashConfirmProps {
  readonly target: TrashTarget;
  /** The record's display name, which the confirm and the list screen's notice carry. */
  readonly name: string;
  /** Facts about the record that its own page counts, shown before the links. */
  readonly detail?: ReactNode;
  readonly onCancel: () => void;
}

export const TrashConfirm = ({ target, name, detail = null, onCancel }: TrashConfirmProps): ReactElement => {
  const { t } = useTranslator(common, resources, bulkDelete);
  const words = TRASH_KINDS[target.of];
  const [state, dispatch] = useReducer(reduce, { step: 'checking' });
  const mounted = useRef(true);
  const moveButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const { of, id } = target;
  const check = useCallback((): void => {
    dispatch({ type: 'check' });
    Promise.all([bulkDeleteApi.preview({ id: [id] }, of), placesApi.links(id)]).then(
      ([preview, links]) => {
        dispatch({ type: 'checked', facts: { preview, links } });
      },
      (error: unknown) => {
        dispatch({ type: 'uncheckable', refusal: bulkRefusal(toError(error)) });
      },
    );
  }, [of, id]);

  useEffect(check, [check]);

  const mayMove = state.step === 'confirm' && movable(state.facts.preview);
  useEffect(() => {
    if (mayMove) {
      moveButton.current?.focus();
    }
  }, [mayMove]);

  const move = (facts: Facts): void => {
    dispatch({ type: 'move' });
    const run = facts.preview.id;
    bulkApi
      .run(run)
      .then(() => bulkDeleteApi.settled(run))
      .then(
        (settled) => {
          if (!mounted.current) {
            return;
          }
          if (trashed(settled)) {
            rememberTrashed({ run, target, name });
            window.location.hash = words.listHref;
            return;
          }
          dispatch({ type: 'failed', run: settled, error: null });
        },
        (error: unknown) => {
          dispatch({ type: 'failed', run: null, error: toError(error) });
        },
      );
  };

  const cancel = (
    <button type="button" disabled={state.step === 'moving'} onClick={onCancel}>
      {t('action.cancel')}
    </button>
  );

  switch (state.step) {
    case 'checking':
      return (
        <section className="delete-control delete-confirm" aria-busy="true">
          <p role="status">{t(words.checking)}</p>
          <p>{cancel}</p>
        </section>
      );
    case 'unchecked':
      return (
        <section className="delete-control delete-confirm">
          {state.refusal.kind === 'denied' ? (
            <p role="alert">{t(words.denied)}</p>
          ) : (
            <>
              <p role="alert">{t(words.cannotCheck)}</p>
              {state.refusal.kind === 'error' ? <ErrorNotice error={state.refusal.error} /> : null}
            </>
          )}
          <p>
            {state.refusal.kind === 'denied' ? null : (
              <button type="button" onClick={check}>
                {t('bulkDelete.item.checkAgain')}
              </button>
            )}
            {' '}
            {cancel}
          </p>
        </section>
      );
    case 'confirm':
    case 'moving': {
      const busy = state.step === 'moving';
      const facts = state.facts;
      return (
        <section className="delete-control delete-confirm" aria-label={t('bulkDelete.item.move')}>
          {facts.preview.total === 0 ? (
            <p role="alert">{t('linksPlace.unavailable')}</p>
          ) : (
            <Facts target={target} name={name} facts={facts} detail={detail} />
          )}
          {facts.preview.denied > 0 ? <p role="alert">{t(words.denied)}</p> : null}
          <p>
            {movable(facts.preview) ? (
              <button
                ref={moveButton}
                type="button"
                className="delete-confirm-button"
                disabled={busy}
                aria-busy={busy}
                onClick={() => {
                  move(facts);
                }}
              >
                {busy ? t('bulkDelete.item.moving') : t('bulkDelete.item.move')}
              </button>
            ) : null}
            {' '}
            {cancel}
          </p>
        </section>
      );
    }
    case 'failed':
      return (
        <section className="delete-control delete-confirm">
          <p>{t('bulkDelete.item.failed', { name })}</p>
          <Failure run={state.run} error={state.error} />
          <p>
            <button type="button" onClick={check}>
              {t('bulkDelete.item.checkAgain')}
            </button>
            {' '}
            {cancel}
          </p>
        </section>
      );
  }
};

export default TrashConfirm;
