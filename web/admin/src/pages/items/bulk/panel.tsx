/**
 * The Items list's bulk change panel: one change to the ticked items or to every item the list
 * matches, its preview, its progress, and its undo. The steps are `model.ts`; this file sends the
 * requests each step asks for and reads a running change again every 2 seconds.
 *
 * It is its own chunk, loaded when the person opens it, with its own message namespace.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useReducer, useRef } from 'react';
import type { BulkQuery, BulkRefusal, BulkRun } from '../../../api/bulk';
import { bulkApi, bulkRefusal, isLive, matchingQuery, tickedQuery } from '../../../api/bulk';
import type { ResourceQuery } from '../../../api/client';
import type { ItemId } from '../../../api/types';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import type { BrowseOptions } from '../browse-options';
import { EditStep } from './edit-step';
import { CLOSED, isBusy, isPolling, reduce } from './model';
import type { Action, Draft, Panel } from './model';
import { PreviewStep } from './preview-step';
import { UndoStep, WatchStep } from './run-step';
import type { Reach } from './scope-choice';
import './bulk-panel.css';

/** The delete's steps and the recent list are their own chunks, so the panel stays in budget. */
const DeleteStep = lazy(() => import('./delete-step').then((module) => ({ default: module.DeleteStep })));
const RecentStep = lazy(() => import('./recent-step').then((module) => ({ default: module.RecentStep })));

export const POLL_MS = 2000;

export interface BulkPanelProps {
  readonly start: 'edit' | 'recent' | 'delete';
  readonly picked: readonly ItemId[];
  /** The list's query; its filters are what "all that match" sends. */
  readonly query: ResourceQuery;
  readonly reach: Reach;
  readonly options: BrowseOptions;
  readonly onClose: () => void;
  /** A change this panel started has ended: the list reads its page again. */
  readonly onChanged: () => void;
}

const firstStep = (props: BulkPanelProps): Panel =>
  reduce(
    CLOSED,
    props.start === 'edit'
      ? { type: 'openEdit', scope: props.picked.length > 0 ? 'ticked' : 'matching' }
      : { type: props.start === 'recent' ? 'openRecent' : 'openDelete' },
  );

/** The panel's state and the requests that move it. */
const useBulkPanel = (props: BulkPanelProps) => {
  const [panel, dispatch] = useReducer(reduce, props, firstStep);
  /** Set when this panel starts a run or an undo, so its end refreshes the list once. */
  const started = useRef(false);
  const onChanged = useRef(props.onChanged);
  onChanged.current = props.onChanged;

  const attempt = (sent: Action, work: () => Promise<Action>): void => {
    dispatch(sent);
    work().then(dispatch, (error: unknown) => {
      dispatch({ type: 'refused', refusal: bulkRefusal(toError(error)) });
    });
  };
  const queryOf = (draft: Draft): BulkQuery =>
    draft.scope === 'ticked' ? tickedQuery(props.picked) : matchingQuery(props.query);
  const startThen = async (id: BulkRun['id'], begin: () => Promise<void>): Promise<Action> => {
    await begin();
    started.current = true;
    return { type: 'run', run: await bulkApi.get(id) };
  };

  useEffect(() => {
    if (panel.step !== 'recent' || panel.runs !== null || panel.failure !== null) {
      return;
    }
    bulkApi.recent().then(
      (runs) => {
        dispatch({ type: 'recentLoaded', runs });
      },
      (error: unknown) => {
        dispatch({ type: 'refused', refusal: bulkRefusal(toError(error)) });
      },
    );
  }, [panel]);

  useEffect(() => {
    if (panel.step === 'watch' && !isLive(panel.run.status) && started.current) {
      started.current = false;
      onChanged.current();
    }
    if (!isPolling(panel) || panel.step !== 'watch') {
      return undefined;
    }
    const id = panel.run.id;
    const timer = window.setTimeout(() => {
      bulkApi.get(id).then(
        (run) => {
          dispatch({ type: 'run', run });
        },
        (error: unknown) => {
          dispatch({ type: 'refused', refusal: bulkRefusal(toError(error)) });
        },
      );
    }, POLL_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [panel]);

  const commands = {
    dispatch,
    started: (run: BulkRun): void => {
      started.current = true;
      dispatch({ type: 'run', run });
    },
    refreshRecent: (): void => {
      dispatch({ type: 'openRecent' });
    },
    preview: (draft: Draft): void => {
      attempt({ type: 'previewSent' }, async () => ({
        type: 'previewed',
        preview: await bulkApi.preview(queryOf(draft), draft.edit),
      }));
    },
    apply: (id: BulkRun['id']): void => {
      attempt({ type: 'runSent' }, () => startThen(id, () => bulkApi.run(id)));
    },
    goOn: (id: BulkRun['id']): void => {
      attempt({ type: 'resumeSent' }, () => startThen(id, () => bulkApi.run(id)));
    },
    stop: (id: BulkRun['id']): void => {
      bulkApi.stop(id).then(
        () => bulkApi.get(id).then((run) => {
          dispatch({ type: 'run', run });
        }),
        (error: unknown) => {
          dispatch({ type: 'refused', refusal: bulkRefusal(toError(error)) });
        },
      );
    },
    undoCheck: (run: BulkRun): void => {
      attempt({ type: 'undoCheck', run }, async () => ({
        type: 'undoChecked',
        check: await bulkApi.undoPreview(run.id),
      }));
    },
    undo: (id: BulkRun['id'], force: boolean): void => {
      attempt({ type: 'undoSent' }, () => startThen(id, () => bulkApi.undo(id, force)));
    },
  };
  return { panel, commands };
};

const Refusal = ({ refusal }: { readonly refusal: BulkRefusal | null }): ReactElement | null => {
  const { t } = useTranslator(bulk);
  if (refusal === null || refusal.kind === 'expired') {
    return null;
  }
  return refusal.kind === 'denied' ? <p role="alert">{t('bulk.refused.denied')}</p> : <ErrorNotice error={refusal.error} />;
};

const failureOf = (panel: Panel): BulkRefusal | null =>
  'failure' in panel ? panel.failure : null;

const Step = ({
  panel,
  props,
  commands,
}: {
  readonly panel: Panel;
  readonly props: BulkPanelProps;
  readonly commands: ReturnType<typeof useBulkPanel>['commands'];
}): ReactElement | null => {
  const busy = isBusy(panel);
  const { dispatch } = commands;
  const { t } = useTranslator(bulk);
  switch (panel.step) {
    case 'closed':
      return null;
    case 'delete':
      return (
        <Suspense fallback={<p role="status">{t('bulk.recent.loading')}</p>}>
          <DeleteStep
            picked={props.picked}
            query={props.query}
            reach={props.reach}
            onStarted={commands.started}
            onCancel={props.onClose}
          />
        </Suspense>
      );
    case 'recent':
      return (
        <Suspense fallback={<p role="status">{t('bulk.recent.loading')}</p>}>
          <RecentStep
            runs={panel.runs}
            options={props.options}
            busy={busy}
            onShow={(run) => {
              dispatch({ type: 'show', run });
            }}
            onUndo={commands.undoCheck}
            onEmptied={commands.refreshRecent}
          />
        </Suspense>
      );
    case 'edit':
    case 'previewing':
      return (
        <EditStep
          draft={panel.draft}
          reach={props.reach}
          options={props.options}
          busy={busy}
          onDraft={(draft) => {
            dispatch({ type: 'draft', draft });
          }}
          onPreview={() => {
            commands.preview(panel.draft);
          }}
          onCancel={props.onClose}
        />
      );
    case 'preview':
    case 'starting':
      return (
        <PreviewStep
          preview={panel.preview}
          options={props.options}
          expired={panel.step === 'preview' && panel.failure?.kind === 'expired'}
          busy={busy}
          onApply={() => {
            commands.apply(panel.preview.id);
          }}
          onPreviewAgain={() => {
            commands.preview(panel.draft);
          }}
          onBack={() => {
            dispatch({ type: 'back' });
          }}
        />
      );
    case 'watch':
    case 'resuming':
    case 'undoCheck':
      return (
        <WatchStep
          run={panel.run}
          busy={busy}
          onStop={() => {
            commands.stop(panel.run.id);
          }}
          onGoOn={() => {
            commands.goOn(panel.run.id);
          }}
          onUndo={() => {
            commands.undoCheck(panel.run);
          }}
        />
      );
    case 'undoAsk':
    case 'undoStarting':
      return (
        <UndoStep
          check={panel.check}
          force={panel.force}
          busy={busy}
          onForce={(force) => {
            dispatch({ type: 'force', force });
          }}
          onUndo={() => {
            commands.undo(panel.run.id, panel.force);
          }}
          onBack={() => {
            dispatch({ type: 'back' });
          }}
        />
      );
  }
};

const BulkPanel = (props: BulkPanelProps): ReactElement => {
  const { t } = useTranslator(bulk);
  const { panel, commands } = useBulkPanel(props);
  const heading =
    panel.step === 'recent'
      ? t('bulk.recent.heading')
      : props.start === 'delete'
        ? t('bulk.panel.delete')
        : t('bulk.panel.label');
  return (
    <section className="bulk-panel" aria-label={heading}>
      <p className="bulk-panel-head">
        <strong>{heading}</strong>
        <button type="button" className="link-button" onClick={props.onClose}>
          {t('bulk.panel.close')}
        </button>
      </p>
      <Step panel={panel} props={props} commands={commands} />
      <Refusal refusal={failureOf(panel)} />
    </section>
  );
};

export default BulkPanel;
