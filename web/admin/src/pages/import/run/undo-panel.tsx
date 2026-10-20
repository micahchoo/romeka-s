/**
 * Undo, before it starts: what it removes and puts back, read from the server
 * without writing (`GET …/undo/preview`), and the resources it keeps because
 * something changed them after the import, each with what changed it. The
 * person may undo any of those anyway, one by one or all; the sentence under
 * the list says what that loses before the press (`docs/import-v2.md`,
 * "Undo").
 */
import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { conflictOf } from '../../../api/import-conflict';
import { importApi } from '../../../api/import-routes';
import type { Force } from '../../../api/import-routes';
import type { KeptResource, NamedResource, UndoPreview } from '../../../api/import-undo';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import type { AsyncState } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { PagedList } from '../paged-list';
import { pageOf } from './page-of';
import { useRunText } from './messages';
import { RefusalNotice, refusalOf } from './refusal';
import type { Refusal } from './refusal';
import type { LiveImport } from './use-live-import';
import { useServerWords } from './use-server-words';
import { kindCountsWords, resourceHref, resourceWords } from './words';

const keyOf = (resource: NamedResource): string => `${resource.kind}:${String(resource.id)}`;

/** Undoing these anyway deletes their values on resources outside the import. */
const SHARED_KINDS: ReadonlySet<string> = new Set(['resourceTemplate', 'property']);

/** The resources an undo acts on although changed: none, every kept one, or the ones ticked. */
export type Chosen = { readonly all: true } | { readonly all: false; readonly keys: ReadonlySet<string> };

export const forceOf = (chosen: Chosen, kept: readonly KeptResource[]): Force | undefined => {
  if (chosen.all) {
    return kept.length === 0 ? undefined : 'all';
  }
  const named = kept.filter((resource) => chosen.keys.has(keyOf(resource))).map(({ kind, id }) => ({ kind, id }));
  return named.length === 0 ? undefined : named;
};

const KeptList = ({
  kept,
  chosen,
  onChoose,
}: {
  readonly kept: readonly KeptResource[];
  readonly chosen: Chosen;
  readonly onChoose: (next: Chosen) => void;
}): ReactElement => {
  const translator = useRunText();
  const { t, number } = translator;
  const server = useServerWords();
  const ticked = (resource: KeptResource): boolean => chosen.all || chosen.keys.has(keyOf(resource));
  const toggle = (resource: KeptResource, on: boolean): void => {
    const keys = new Set(chosen.all ? kept.map(keyOf) : chosen.keys);
    if (on) {
      keys.add(keyOf(resource));
    } else {
      keys.delete(keyOf(resource));
    }
    onChoose({ all: false, keys });
  };
  return (
    <>
      <p>{kept.length === 1 ? t('run.undo.keptOne') : t('run.undo.kept', { count: number(kept.length) })}</p>
      <PagedList<KeptResource>
        label={t('run.undo.list')}
        resource={t('run.undo.list')}
        load={pageOf(kept)}
        deps={[kept]}
        columns={[
          {
            header: t('run.end.what'),
            cell: (resource) => <a href={resourceHref(resource.kind, resource.id)}>{resourceWords(translator, resource.kind, resource.id)}</a>,
          },
          {
            header: t('run.end.why'),
            cell: (resource) => (
              <ul className="import-run-reasons">
                {resource.reasons.map((reason) => (
                  <li key={reason.reason}>
                    {server(reason.detail)}
                    {reason.import === undefined ? null : (
                      <>
                        {' '}
                        <a href={`#/import/${String(reason.import)}`}>{`#${String(reason.import)}`}</a>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            ),
          },
          {
            header: t('run.undo.anyway'),
            cell: (resource) => (
              <input
                type="checkbox"
                aria-label={`${t('run.undo.anyway')}: ${resourceWords(translator, resource.kind, resource.id)}`}
                checked={ticked(resource)}
                onChange={(event) => {
                  toggle(resource, event.target.checked);
                }}
              />
            ),
          },
        ]}
        rowKey={keyOf}
        empty={null}
      />
      {kept.length > 1 ? (
        <p>
          <label>
            <input
              type="checkbox"
              checked={chosen.all}
              onChange={(event) => {
                onChoose(event.target.checked ? { all: true } : { all: false, keys: new Set() });
              }}
            />{' '}
            {t('run.undo.all', { count: number(kept.length) })}
          </label>
        </p>
      ) : null}
    </>
  );
};

/** What the chosen ones lose, said before the press. */
const Loss = ({ kept, chosen }: { readonly kept: readonly KeptResource[]; readonly chosen: Chosen }): ReactElement | null => {
  const { t, number } = useRunText();
  const picked = chosen.all ? kept : kept.filter((resource) => chosen.keys.has(keyOf(resource)));
  if (picked.length === 0) {
    return null;
  }
  return (
    <div className="import-run-loss" role="status">
      <p>{picked.length === 1 ? t('run.undo.lossOne') : t('run.undo.loss', { count: number(picked.length) })}</p>
      {picked.some((resource) => SHARED_KINDS.has(resource.kind)) ? <p>{t('run.undo.lossTypes')}</p> : null}
    </div>
  );
};

type Sending =
  | { readonly status: 'idle' | 'sending' }
  | { readonly status: 'refused'; readonly refusal: Refusal }
  | { readonly status: 'failed'; readonly error: Error };

const Opened = ({ live, onCancel }: { readonly live: LiveImport; readonly onCancel: (() => void) | null }): ReactElement => {
  const draft = useDraft();
  const translator = useRunText();
  const { t, number } = translator;
  const [preview, setPreview] = useState<AsyncState<UndoPreview>>({ status: 'loading' });
  const [chosen, setChosen] = useState<Chosen>({ all: false, keys: new Set() });
  const [sending, setSending] = useState<Sending>({ status: 'idle' });
  const { refresh } = live;

  useEffect(() => {
    let current = true;
    importApi.undoPreview(draft.id).then(
      (data) => {
        if (current) {
          setPreview({ status: 'ready', data });
        }
      },
      (error: unknown) => {
        if (current) {
          setPreview({ status: 'error', error: toError(error) });
          // A refusal says the page shows where the import stands, so read it.
          if (conflictOf(error) !== null) {
            refresh();
          }
        }
      },
    );
    return () => {
      current = false;
    };
  }, [draft.id, refresh]);

  if (preview.status === 'loading') {
    return (
      <p role="status" className="import-slow">
        {t('run.undo.reading')}
      </p>
    );
  }
  if (preview.status === 'error') {
    const conflict = conflictOf(preview.error);
    const refusal = conflict === null ? null : refusalOf(conflict, draft.id, translator);
    return refusal === null ? (
      <ErrorNotice error={preview.error} />
    ) : (
      <RefusalNotice refusal={refusal} id={draft.id} mode={draft.place.mode} onAgain={live.refresh} />
    );
  }
  const { remove, restore, kept, skipped } = preview.data;
  const removes = kindCountsWords(translator, remove);
  const restores = kindCountsWords(translator, restore);
  const force = forceOf(chosen, kept);
  const undo = (): void => {
    setSending({ status: 'sending' });
    importApi.undo(draft.id, force).then(
      () => {
        setSending({ status: 'idle' });
        live.refresh();
      },
      (error: unknown) => {
        const conflict = conflictOf(error);
        const refusal = conflict === null ? null : refusalOf(conflict, draft.id, translator);
        setSending(refusal === null ? { status: 'failed', error: toError(error) } : { status: 'refused', refusal });
        if (refusal?.next.kind === 'refresh') {
          live.refresh();
        }
      },
    );
  };
  const nothing = removes === null && restores === null && force === undefined;
  return (
    <div className="import-run-undo">
      {removes === null ? null : <p>{t('run.undo.removes', { what: removes })}</p>}
      {restores === null ? null : <p>{t('run.undo.restores', { what: restores })}</p>}
      {nothing ? <p>{t('run.undo.empty')}</p> : null}
      {skipped.length === 0 ? null : (
        <p>{skipped.length === 1 ? t('run.undo.goneOne') : t('run.undo.gone', { count: number(skipped.length) })}</p>
      )}
      {kept.length === 0 ? null : <KeptList kept={kept} chosen={chosen} onChoose={setChosen} />}
      <Loss kept={kept} chosen={chosen} />
      <p className="import-run-actions">
        <button
          type="button"
          className="import-run-danger"
          disabled={nothing || sending.status === 'sending'}
          aria-busy={sending.status === 'sending'}
          onClick={undo}
        >
          {sending.status === 'sending' ? t('run.undo.starting') : t('run.undo.go')}
        </button>
        {onCancel === null ? null : (
          <button type="button" onClick={onCancel}>
            {t('run.undo.cancel')}
          </button>
        )}
      </p>
      {sending.status === 'refused' ? (
        <RefusalNotice refusal={sending.refusal} id={draft.id} mode={draft.place.mode} onAgain={undo} />
      ) : null}
      {sending.status === 'failed' ? <ErrorNotice error={sending.error} /> : null}
    </div>
  );
};

/** "Undo this import...", which opens the panel; or the panel open at once, with no way to close it. */
export const UndoPanel = ({
  live,
  opener,
  open = false,
}: {
  readonly live: LiveImport;
  /** The words of the button that opens the panel. */
  readonly opener: string;
  readonly open?: boolean;
}): ReactElement => {
  const [opened, setOpened] = useState(open);
  if (!opened) {
    return (
      <p>
        <button
          type="button"
          onClick={() => {
            setOpened(true);
          }}
        >
          {opener}
        </button>
      </p>
    );
  }
  return (
    <Opened
      live={live}
      onCancel={
        open
          ? null
          : () => {
              setOpened(false);
            }
      }
    />
  );
};
