/**
 * The bulk delete's own steps, before its run: which items, the preview of what goes, and the
 * confirm, which needs the number of items typed. Once the run starts the panel watches it as it
 * watches an edit.
 *
 * ```text
 * ask -> previewing -> confirm -> starting -> (the panel's watch)
 *        confirm -(back)-> ask
 * ```
 *
 * It is its own chunk, loaded when the person opens Delete, with its own message namespace.
 */
import type { ReactElement } from 'react';
import { useId, useReducer } from 'react';
import type { BulkRefusal, BulkRun } from '../../../api/bulk';
import { bulkApi, bulkRefusal, matchingQuery, tickedQuery } from '../../../api/bulk';
import type { BulkDeletePreview } from '../../../api/bulk-delete';
import { bulkDeleteApi } from '../../../api/bulk-delete';
import type { ResourceQuery } from '../../../api/client';
import type { ItemId } from '../../../api/types';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import type { Locale } from '../../../i18n';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { bulkDelete } from '../../../i18n/namespaces/bulk-delete';
import type { Scope } from './model';
import { ScopeChoice } from './scope-choice';
import type { Reach } from './scope-choice';

type State =
  | { readonly step: 'ask'; readonly scope: Scope; readonly failure: BulkRefusal | null }
  | { readonly step: 'previewing'; readonly scope: Scope }
  | {
      readonly step: 'confirm';
      readonly scope: Scope;
      readonly preview: BulkDeletePreview;
      readonly typed: string;
      readonly failure: BulkRefusal | null;
    }
  | { readonly step: 'starting'; readonly scope: Scope; readonly preview: BulkDeletePreview; readonly typed: string };

type Action =
  | { readonly type: 'scope'; readonly scope: Scope }
  | { readonly type: 'previewSent' }
  | { readonly type: 'previewed'; readonly preview: BulkDeletePreview }
  | { readonly type: 'typed'; readonly typed: string }
  | { readonly type: 'back' }
  | { readonly type: 'startSent' }
  | { readonly type: 'refused'; readonly refusal: BulkRefusal };

const reduce = (state: State, action: Action): State => {
  switch (action.type) {
    case 'scope':
      return state.step === 'ask' ? { step: 'ask', scope: action.scope, failure: null } : state;
    case 'previewSent':
      return state.step === 'ask' || state.step === 'confirm' ? { step: 'previewing', scope: state.scope } : state;
    case 'previewed':
      return state.step === 'previewing'
        ? { step: 'confirm', scope: state.scope, preview: action.preview, typed: '', failure: null }
        : state;
    case 'typed':
      return state.step === 'confirm' ? { ...state, typed: action.typed, failure: null } : state;
    case 'back':
      return state.step === 'confirm' ? { step: 'ask', scope: state.scope, failure: null } : state;
    case 'startSent':
      return state.step === 'confirm'
        ? { step: 'starting', scope: state.scope, preview: state.preview, typed: state.typed }
        : state;
    case 'refused':
      switch (state.step) {
        case 'previewing':
          return { step: 'ask', scope: state.scope, failure: action.refusal };
        case 'starting':
          return { step: 'confirm', scope: state.scope, preview: state.preview, typed: state.typed, failure: action.refusal };
        default:
          return { ...state, failure: action.refusal };
      }
  }
};

const UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;

/** A file size in the largest unit that keeps it at 1 or more, in the reader's locale. */
const sizeText = (bytes: number, locale: Locale): string => {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: 'unit',
    unit: UNITS[unit],
    unitDisplay: 'short',
    maximumFractionDigits: 1,
  }).format(value);
};

const Refused = ({ refusal }: { readonly refusal: BulkRefusal | null }): ReactElement | null => {
  const { t } = useTranslator(bulk);
  if (refusal === null) {
    return null;
  }
  switch (refusal.kind) {
    case 'expired':
      return <p role="alert">{t('bulk.preview.expired')}</p>;
    case 'denied':
      return <p role="alert">{t('bulk.refused.denied')}</p>;
    case 'error':
      return <ErrorNotice error={refusal.error} />;
  }
};

/** What the preview found, in words, and the typed confirm. */
const Confirm = ({
  state,
  onTyped,
  onStart,
  onPreviewAgain,
  onBack,
}: {
  readonly state: Extract<State, { readonly step: 'confirm' | 'starting' }>;
  readonly onTyped: (typed: string) => void;
  readonly onStart: () => void;
  readonly onPreviewAgain: () => void;
  readonly onBack: () => void;
}): ReactElement => {
  const { t, number, locale } = useTranslator(bulk, bulkDelete);
  const inputId = useId();
  const { preview } = state;
  const goes = preview.total - preview.denied;
  const busy = state.step === 'starting';
  const expired = state.step === 'confirm' && state.failure?.kind === 'expired';
  const size = sizeText(preview.bytes, locale);
  const count = number(goes);
  return (
    <div className="bulk-preview">
      <p className="bulk-headline" role="status">
        {goes === 0
          ? t('bulkDelete.confirm.none')
          : goes === 1
            ? t('bulkDelete.confirm.itemsOne')
            : t('bulkDelete.confirm.itemsMany', { count })}
      </p>
      {preview.media === 1 ? <p>{t('bulkDelete.confirm.mediaOne', { size })}</p> : null}
      {preview.media > 1 ? <p>{t('bulkDelete.confirm.mediaMany', { count: number(preview.media), size })}</p> : null}
      {preview.denied === 1 ? <p>{t('bulkDelete.confirm.deniedOne')}</p> : null}
      {preview.denied > 1 ? <p>{t('bulkDelete.confirm.deniedMany', { count: number(preview.denied) })}</p> : null}
      {preview.samples.length === 0 ? null : (
        <section aria-label={t('bulkDelete.confirm.samples')}>
          <p>{t('bulkDelete.confirm.samples')}:</p>
          <ul className="bulk-items">
            {preview.samples.map((sample) => (
              <li key={String(sample.id)}>
                <a href={`#/items/${String(sample.id)}`}>{sample.title ?? t('bulk.word.untitled', { id: String(sample.id) })}</a>
              </li>
            ))}
          </ul>
        </section>
      )}
      {goes === 0 ? null : <p>{t('bulkDelete.confirm.trash', { days: number(preview.trashDays) })}</p>}
      <Refused refusal={state.step === 'confirm' ? state.failure : null} />
      {expired ? (
        <p className="bulk-actions">
          <button type="button" onClick={onPreviewAgain}>
            {t('bulk.preview.again')}
          </button>
        </p>
      ) : goes === 0 ? null : (
        <form
          className="bulk-confirm"
          onSubmit={(event) => {
            event.preventDefault();
            if (state.typed.trim() === String(goes)) {
              onStart();
            }
          }}
        >
          <p className="bulk-field">
            <label htmlFor={inputId}>{t('bulkDelete.confirm.type', { count })}</label>
            <input
              id={inputId}
              inputMode="numeric"
              autoComplete="off"
              value={state.typed}
              disabled={busy}
              onChange={(event) => {
                onTyped(event.target.value);
              }}
            />
          </p>
          <p className="bulk-actions">
            <button type="submit" disabled={busy || state.typed.trim() !== String(goes)} aria-busy={busy}>
              {goes === 1 ? t('bulkDelete.confirm.moveOne') : t('bulkDelete.confirm.moveMany', { count })}
            </button>
          </p>
        </form>
      )}
      <p className="bulk-actions">
        <button type="button" className="link-button" disabled={busy} onClick={onBack}>
          {t('bulk.preview.back')}
        </button>
      </p>
    </div>
  );
};

export const DeleteStep = ({
  picked,
  query,
  reach,
  onStarted,
  onCancel,
}: {
  readonly picked: readonly ItemId[];
  /** The list's query; its filters are what "all that match" sends. */
  readonly query: ResourceQuery;
  readonly reach: Reach;
  /** The run has started; the panel watches it from here. */
  readonly onStarted: (run: BulkRun) => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const { t } = useTranslator(bulk, bulkDelete);
  const [state, dispatch] = useReducer(reduce, {
    step: 'ask',
    scope: picked.length > 0 ? 'ticked' : 'matching',
    failure: null,
  });
  const refuse = (error: unknown): void => {
    dispatch({ type: 'refused', refusal: bulkRefusal(toError(error)) });
  };
  const preview = (scope: Scope): void => {
    dispatch({ type: 'previewSent' });
    const selected = scope === 'ticked' ? tickedQuery(picked) : matchingQuery(query);
    bulkDeleteApi.preview(selected).then((answer) => {
      dispatch({ type: 'previewed', preview: answer });
    }, refuse);
  };
  if (state.step === 'confirm' || state.step === 'starting') {
    const id = state.preview.id;
    return (
      <Confirm
        state={state}
        onTyped={(typed) => {
          dispatch({ type: 'typed', typed });
        }}
        onStart={() => {
          dispatch({ type: 'startSent' });
          bulkApi
            .run(id)
            .then(() => bulkApi.get(id))
            .then(onStarted, refuse);
        }}
        onPreviewAgain={() => {
          preview(state.scope);
        }}
        onBack={() => {
          dispatch({ type: 'back' });
        }}
      />
    );
  }
  const busy = state.step === 'previewing';
  const ready = state.scope === 'ticked' ? reach.ticked > 0 : reach.canMatchAll;
  return (
    <form
      className="bulk-edit"
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) {
          preview(state.scope);
        }
      }}
    >
      <ScopeChoice
        scope={state.scope}
        reach={reach}
        refused={t('bulkDelete.scope.refused')}
        onScope={(scope) => {
          dispatch({ type: 'scope', scope });
        }}
      />
      <p className="bulk-note">{t('bulkDelete.ask.note')}</p>
      <Refused refusal={state.step === 'ask' ? state.failure : null} />
      <p className="bulk-actions">
        <button type="submit" disabled={!ready || busy} aria-busy={busy}>
          {t('bulk.edit.preview')}
        </button>
        <button type="button" className="link-button" onClick={onCancel}>
          {t('bulk.edit.cancel')}
        </button>
      </p>
    </form>
  );
};
