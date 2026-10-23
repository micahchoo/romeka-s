/**
 * One dialog for adding and for replacing captions.
 *
 * Both are the same POST (contract 6.5), so they are one form: the file, the
 * language, and the effect the server would have, read from a dry run before
 * the editor commits. The commit button names what it will do, and the result
 * line states whether rows change and how many of them were changed here by
 * hand, because a replace deletes the language's rows and only the last
 * replace can be undone.
 */
import type { ChangeEvent, ReactElement } from 'react';
import { useEffect, useId, useRef, useState } from 'react';
import { formatBytes } from '../../../app/format';
import { languageName } from '../../../app/language';
import type { MediaId } from '../../../api/types';
import { formatError } from '../../../app/format';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { LOCALES } from '../../../i18n/locale';
import { notesClient } from './client';
import type { TrackImport, TrackPreview, TrackSummary } from './client';
import { clockText } from './time-text';

/** The largest caption file the server reads (contract 6.5). */
const MAX_BYTES = 1024 * 1024;

/** A language tag: `en`, `pt-BR`, `zh-Hant-TW`. */
const LANGUAGE_TAG = /^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$/;

/** What the dialog is doing. */
type Mode = { readonly replace: TrackSummary | null };

export interface CaptionDialogProps {
  readonly mediaId: MediaId;
  readonly mode: Mode;
  /** The languages already on the media, offered before the admin's own. */
  readonly existing: readonly string[];
  /** The language to start on: the row's, else the media's, else none. */
  readonly defaultLang: string | null;
  /** A file already chosen, for **Use as captions of…**. */
  readonly file: File | null;
  readonly onClose: () => void;
  readonly onImported: (imported: TrackImport) => void;
}

const OTHER = '__other';

export const CaptionDialog = (props: CaptionDialogProps): ReactElement => {
  const translator = useTranslator(annotations);
  const { t, locale } = translator;
  const headingId = useId();
  const element = useRef<HTMLDialogElement>(null);
  const [file, setFile] = useState<File | null>(props.file);
  const [choice, setChoice] = useState<string>(
    props.defaultLang === null ? '' : props.defaultLang,
  );
  const [otherCode, setOtherCode] = useState('');
  const [preview, setPreview] = useState<TrackPreview | null>(null);
  const [readFailed, setReadFailed] = useState<Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [tooBig, setTooBig] = useState(false);

  const other = choice === OTHER;
  const lang = other ? otherCode.trim() : choice === '' ? null : choice;
  const langValid = lang === null || lang === '' || LANGUAGE_TAG.test(lang);
  const codes = [...new Set([...props.existing, ...LOCALES, ...(props.defaultLang === null ? [] : [props.defaultLang])])];

  // A browser with `showModal` gets a real modal: the page behind it is inert
  // and Esc reaches this dialog first. A test runtime without it keeps the
  // `open` attribute, so the same markup renders as an inline panel.
  useEffect(() => {
    const dialog = element.current;
    if (dialog === null) {
      return;
    }
    try {
      if (typeof dialog.showModal === 'function' && !dialog.open) {
        dialog.showModal();
      }
    } catch {
      // Already open, or no top layer in this runtime.
    }
  }, []);

  useEffect(() => {
    if (file === null || file.size > MAX_BYTES || !langValid) {
      setPreview(null);
      setReadFailed(null);
      return;
    }
    let live = true;
    setPreview(null);
    setReadFailed(null);
    notesClient.previewTrack(props.mediaId, file, lang === '' ? null : lang).then(
      (answer) => {
        if (live) {
          setPreview(answer);
        }
      },
      (failure: unknown) => {
        if (live) {
          setReadFailed(failure instanceof Error ? failure : new Error(String(failure)));
        }
      },
    );
    return () => {
      live = false;
    };
  }, [file, lang, langValid, props.mediaId]);

  const choose = (event: ChangeEvent<HTMLInputElement>): void => {
    const chosen = event.target.files?.[0] ?? null;
    setFile(chosen);
    setError(null);
    setTooBig(chosen !== null && chosen.size > MAX_BYTES);
  };

  const commit = (): void => {
    if (busy) {
      return;
    }
    if (file === null) {
      setError(new Error(t('captions.dialog.noFile')));
      return;
    }
    if (file.size > MAX_BYTES) {
      setTooBig(true);
      return;
    }
    if (!langValid) {
      return;
    }
    setBusy(true);
    setError(null);
    notesClient.importTrack(props.mediaId, file, lang === '' ? null : lang).then(
      (imported) => {
        setBusy(false);
        props.onImported(imported);
      },
      (failure: unknown) => {
        setBusy(false);
        setError(failure instanceof Error ? failure : new Error(String(failure)));
      },
    );
  };

  const language =
    lang === null || lang === '' ? t('captions.dialog.noLanguage') : languageName(lang, locale);
  const replacing = preview !== null ? preview.replaces : (props.mode.replace?.cues ?? 0);
  const label =
    replacing > 1
      ? t('captions.dialog.commitReplace', { cues: replacing, language })
      : replacing === 1
        ? t('captions.dialog.commitReplaceOne', { language })
        : t('captions.dialog.commitAdd', { language });

  return (
    <dialog
      ref={element}
      open
      className="caption-dialog"
      aria-labelledby={headingId}
      onCancel={(event) => {
        event.preventDefault();
        props.onClose();
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          props.onClose();
        }
      }}
    >
      <h3 id={headingId}>
        {props.mode.replace === null
          ? t('captions.dialog.titleAdd')
          : t('captions.dialog.titleReplace')}
      </h3>

      <label className="note-field">
        <span>{t('captions.dialog.file')}</span>
        <input
          type="file"
          accept=".vtt,.srt,text/vtt,application/x-subrip"
          autoFocus
          onChange={choose}
        />
        <small>{t('captions.dialog.fileHint')}</small>
      </label>
      {file === null ? null : (
        <p className="caption-file">
          {file.name}, {formatBytes(file.size, translator)}
        </p>
      )}
      {tooBig && file !== null ? (
        <p className="note-field-error" role="alert">
          {t('captions.dialog.tooBig', { size: formatBytes(file.size, translator) })}
        </p>
      ) : null}

      <label className="note-field">
        <span>{t('captions.dialog.language')}</span>
        <select
          value={choice}
          onChange={(event) => {
            setChoice(event.target.value);
          }}
        >
          <option value="">{t('captions.dialog.noLanguage')}</option>
          {codes.map((code) => (
            <option key={code} value={code}>
              {languageName(code, locale)}
            </option>
          ))}
          <option value={OTHER}>{t('captions.dialog.other')}</option>
        </select>
      </label>
      {other ? (
        <label className="note-field">
          <span>{t('captions.dialog.otherLabel')}</span>
          <input
            type="text"
            value={otherCode}
            placeholder="pt-BR"
            aria-invalid={!langValid}
            onChange={(event) => {
              setOtherCode(event.target.value);
            }}
          />
        </label>
      ) : null}
      {langValid ? null : (
        <p className="note-field-error">{t('captions.dialog.otherInvalid')}</p>
      )}

      {preview === null ? (
        readFailed === null ? null : (
          <p className="caption-result">
            {t('captions.dialog.previewFailed')} {formatError(readFailed, translator)}
          </p>
        )
      ) : (
        <div className="caption-result">
          <p>
            {preview.cues === 1
              ? t('captions.dialog.summaryOne', {
                  start: clockText(preview.startMs),
                  end: clockText(preview.endMs),
                })
              : t('captions.dialog.summary', {
                  cues: preview.cues,
                  start: clockText(preview.startMs),
                  end: clockText(preview.endMs),
                })}
          </p>
          {preview.replaces === 0 ? null : (
            <p>
              {preview.replaces === 1
                ? t('captions.dialog.replacesOne', { language: languageName(lang ?? '', locale) })
                : t('captions.dialog.replaces', {
                    cues: preview.replaces,
                    language: languageName(lang ?? '', locale),
                  })}
            </p>
          )}
          {preview.handEdited === 0 ? null : (
            <p>{t('captions.dialog.handEdited', { count: preview.handEdited })}</p>
          )}
        </div>
      )}

      {error === null ? null : (
        <p className="note-field-error" role="alert">
          {formatError(error, translator)}
        </p>
      )}

      <div className="note-form-actions">
        <button type="button" onClick={commit}>
          {busy ? t('captions.dialog.uploading') : label}
        </button>
        <button type="button" onClick={props.onClose}>
          {t('captions.dialog.cancel')}
        </button>
      </div>
    </dialog>
  );
};
