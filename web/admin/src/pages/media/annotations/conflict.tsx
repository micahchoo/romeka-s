/**
 * The 412 view: the version a colleague stored while this editor had the note
 * open, beside the text this editor typed.
 *
 * The view never overwrites or deletes a version the editor has not seen. Save
 * sends this editor's text against their ETag, and Delete names their version,
 * so the action the button performs is the action the sentence describes.
 */
import type { ReactElement } from 'react';
import { useEffect, useId, useRef } from 'react';
import { formatDateTime, formatError } from '../../../app/format';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import type { Draft, Note } from './wire';

export interface ConflictViewProps {
  /** The version the server holds now, with its ETag. */
  readonly theirs: Note;
  readonly draft: Draft;
  /** Whether the action that was refused was a save or a delete. */
  readonly intent: 'save' | 'delete';
  readonly busy: boolean;
  readonly error: Error | null;
  readonly onChange: (draft: Draft) => void;
  readonly onSaveMine: () => void;
  readonly onUseTheirs: () => void;
  readonly onDeleteTheirs: () => void;
  readonly onKeep: () => void;
}

export const ConflictView = (props: ConflictViewProps): ReactElement => {
  const translator = useTranslator(annotations, common);
  const { t, locale } = translator;
  const heading = useRef<HTMLHeadingElement>(null);
  const headingId = useId();

  useEffect(() => {
    heading.current?.focus();
  }, []);

  return (
    <section className="note-conflict" aria-labelledby={headingId}>
      <h4 id={headingId} ref={heading} tabIndex={-1}>
        {t('notes.conflict')}
      </h4>
      <p>
        {t(props.intent === 'save' ? 'notes.conflict.save' : 'notes.conflict.delete')}
      </p>

      <div className="note-conflict-theirs">
        <p className="note-field-label">{t('notes.conflict.theirs')}</p>
        <p className="note-conflict-text">{props.theirs.text}</p>
        {props.theirs.modified === null ? null : (
          <p className="note-meta">
            {t('notes.changed', { when: formatDateTime(props.theirs.modified, locale) })}
          </p>
        )}
      </div>

      {props.intent === 'save' ? (
        <label className="note-field">
          <span>{t('notes.conflict.yours')}</span>
          <textarea
            value={props.draft.text}
            rows={4}
            onChange={(event) => {
              props.onChange({ ...props.draft, text: event.target.value });
            }}
          />
        </label>
      ) : null}

      {props.error === null ? null : (
        <p className="note-field-error" role="alert">
          {t('notes.notSaved')} {formatError(props.error, translator)}
        </p>
      )}

      <div className="note-form-actions">
        {props.intent === 'save' ? (
          <>
            <button
              type="button"
              onClick={() => {
                if (!props.busy) {
                  props.onSaveMine();
                }
              }}
            >
              {props.busy ? t('notes.form.saving') : t('notes.conflict.saveMine')}
            </button>
            <button
              type="button"
              onClick={() => {
                if (!props.busy) {
                  props.onUseTheirs();
                }
              }}
            >
              {t('notes.conflict.useTheirs')}
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className="note-delete"
              onClick={() => {
                if (!props.busy) {
                  props.onDeleteTheirs();
                }
              }}
            >
              {props.busy ? t('delete.busy') : t('notes.conflict.deleteTheirs')}
            </button>
            <button
              type="button"
              onClick={() => {
                if (!props.busy) {
                  props.onKeep();
                }
              }}
            >
              {t('notes.form.keep')}
            </button>
          </>
        )}
      </div>
    </section>
  );
};
