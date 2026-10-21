/**
 * The form that writes one note: its text, and in Advanced its language and the
 * motivation the W3C document calls what the note does.
 *
 * The form holds no draft of its own: the panel owns the draft, so a conflict
 * view and the form show the same text, and session storage keeps it across a
 * reload. The only state here is the two-step delete and whether Save has been
 * busy long enough to say so.
 */
import type { ReactElement } from 'react';
import { useEffect, useRef, useState } from 'react';
import { formatError } from '../../../app/format';
import { useTranslator } from '../../../i18n';
import type { KeyOf } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import type { Draft, Motivation, Note } from './wire';

type AnnotationsMessagesKey = KeyOf<typeof annotations>;

const MOTIVATIONS: readonly (readonly [Motivation, AnnotationsMessagesKey])[] = [
  ['commenting', 'notes.motivation.commenting'],
  ['tagging', 'notes.motivation.tagging'],
  ['describing', 'notes.motivation.describing'],
  ['supplementing', 'notes.motivation.supplementing'],
];

/** How long a save may run before the form says it is still saving. */
const LONG_SAVE_MS = 10_000;

export interface NoteFormProps {
  /** The note being changed, or `null` for a note that is not saved yet. */
  readonly note: Note | null;
  /** What the delete confirmation names this note. */
  readonly rowName: string;
  readonly draft: Draft;
  readonly advanced: boolean;
  readonly busy: boolean;
  readonly error: Error | null;
  readonly restored: boolean;
  readonly onChange: (draft: Draft) => void;
  readonly onSave: () => void;
  readonly onDelete: (() => void) | null;
  readonly onCancel: () => void;
  readonly onAdvanced: (advanced: boolean) => void;
  readonly onDiscardRestored: (() => void) | null;
}

export const NoteForm = (props: NoteFormProps): ReactElement => {
  const translator = useTranslator(annotations, common);
  const { t } = translator;
  const [empty, setEmpty] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [longSave, setLongSave] = useState(false);
  const text = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    text.current?.focus();
  }, []);

  useEffect(() => {
    if (!props.busy) {
      setLongSave(false);
      return;
    }
    const timer = setTimeout(() => {
      setLongSave(true);
    }, LONG_SAVE_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [props.busy]);

  const save = (): void => {
    if (props.busy) {
      return;
    }
    if (props.draft.text.trim() === '') {
      setEmpty(true);
      return;
    }
    setEmpty(false);
    props.onSave();
  };

  const { draft, onChange } = props;
  const remove = props.onDelete;

  return (
    <form
      className="note-form"
      aria-busy={props.busy}
      onSubmit={(event) => {
        event.preventDefault();
        save();
      }}
    >
      <h4 className="note-form-title">
        {props.note === null ? t('notes.form.newTitle') : t('notes.form.editTitle')}
        {props.note === null ? <span className="note-tag"> {t('notes.draft')}</span> : null}
      </h4>

      {props.restored && props.onDiscardRestored !== null ? (
        <p className="note-restored">
          {t('notes.restoredChange')}{' '}
          <button type="button" onClick={props.onDiscardRestored}>
            {t('notes.discard')}
          </button>
        </p>
      ) : null}

      <label className="note-field">
        <span>{t('notes.form.text')}</span>
        <textarea
          ref={text}
          value={draft.text}
          rows={4}
          onChange={(event) => {
            onChange({ ...draft, text: event.target.value });
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
              event.preventDefault();
              save();
            }
          }}
        />
      </label>

      {empty ? (
        <p className="note-field-error" role="alert">
          {t('notes.form.empty')}
        </p>
      ) : null}
      {props.error === null ? null : (
        <p className="note-field-error" role="alert">
          {t('notes.notSaved')} {formatError(props.error, translator)}
        </p>
      )}

      <div className="note-form-actions">
        <button type="submit">{props.busy ? t('notes.form.saving') : t('action.save')}</button>
        <button type="button" onClick={props.onCancel}>
          {t('action.cancel')}
        </button>
        {remove === null ? null : confirming ? (
          <span className="note-delete-confirm" role="group" aria-label={t('notes.form.delete')}>
            <span>{t('delete.confirm', { name: props.rowName })}</span>{' '}
            <button
              type="button"
              className="note-delete"
              onClick={() => {
                if (!props.busy) {
                  remove();
                }
              }}
            >
              {props.busy ? t('delete.busy') : t('notes.form.deleteConfirm')}
            </button>
            <button
              type="button"
              onClick={() => {
                setConfirming(false);
                text.current?.focus();
              }}
            >
              {t('notes.form.keep')}
            </button>
          </span>
        ) : (
          <button
            type="button"
            className="note-delete"
            onClick={() => {
              setConfirming(true);
            }}
          >
            {t('notes.form.delete')}
          </button>
        )}
      </div>

      {longSave ? <p className="note-long-save">{t('notes.form.savingLong')}</p> : null}

      <button
        type="button"
        className="note-advanced-toggle"
        aria-expanded={props.advanced}
        onClick={() => {
          props.onAdvanced(!props.advanced);
        }}
      >
        {t('notes.form.advanced')}
      </button>

      {props.advanced ? (
        <div className="note-advanced-fields">
          <label className="note-field">
            <span>{t('notes.form.motivation')}</span>
            <select
              value={draft.motivation}
              onChange={(event) => {
                const chosen = MOTIVATIONS.find(([motivation]) => motivation === event.target.value);
                if (chosen !== undefined) {
                  onChange({ ...draft, motivation: chosen[0] });
                }
              }}
            >
              {MOTIVATIONS.map(([motivation, key]) => (
                <option key={motivation} value={motivation}>
                  {t(key)}
                </option>
              ))}
            </select>
          </label>
          <label className="note-field">
            <span>{t('notes.form.language')}</span>
            <input
              type="text"
              value={draft.language ?? ''}
              onChange={(event) => {
                onChange({ ...draft, language: event.target.value === '' ? null : event.target.value });
              }}
            />
          </label>
          {props.note === null ? null : (
            <>
              <p className="note-field-label">{t('notes.form.raw')}</p>
              <pre className="note-raw" tabIndex={0}>
                {JSON.stringify(props.note.raw, null, 2)}
              </pre>
            </>
          )}
        </div>
      ) : null}
    </form>
  );
};
