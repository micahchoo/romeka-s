/**
 * Captions: one row per language on a recording, and the dialog that adds,
 * replaces or removes one.
 *
 * The server is the only cue parser, so this section never reads the file. A
 * dry run before a commit states the effect (contract 6.5, G5), which is what
 * makes a replace a decision rather than a surprise: the row's counts are the
 * server's, and the commit button names the count it will replace.
 *
 * The same module carries **Use as captions of…** for a caption file, because
 * that action ends in this dialog too: the file attached to the item is posted
 * to a recording beside it.
 */
import type { ReactElement } from 'react';
import { useCallback, useEffect, useState } from 'react';
import { api } from '../../../api/client';
import type { Media, MediaId } from '../../../api/types';
import { formatDateTime } from '../../../app/format';
import { languageName } from '../../../app/language';
import { mediaKind, mediaName } from '../../../app/media';
import { useTranslator } from '../../../i18n';
import type { KeyOf, Translator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { captionsUrl, notesClient } from './client';
import type { TrackImport, TrackSummary } from './client';
import { CaptionDialog } from './caption-dialog';
import './captions.css';

export interface CaptionsProps {
  readonly mediaId: MediaId;
  readonly writable: boolean;
  /** The media's own language, preselected for a new track. */
  readonly mediaLang: string | null;
  /** The languages and counts the player offers as `<track>` elements. */
  readonly onTracks: (tracks: readonly TrackSummary[]) => void;
}

/** The row's sentence: the count, where it came from and when. */
const rowText = (
  track: TrackSummary,
  when: string | null,
  translate: Translator<KeyOf<typeof annotations>>['t'],
): string => {
  if (track.cues === 1) {
    return when === null ? translate('captions.rowUndatedOne') : translate('captions.rowOne', { when });
  }
  return when === null
    ? translate('captions.rowUndated', { cues: track.cues })
    : translate('captions.row', { cues: track.cues, when });
};

export const Captions = (props: CaptionsProps): ReactElement => {
  const translator = useTranslator(annotations);
  const { t, locale } = translator;
  const [tracks, setTracks] = useState<readonly TrackSummary[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [dialog, setDialog] = useState<{ readonly replace: TrackSummary | null } | null>(null);
  const [removing, setRemoving] = useState<TrackSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const { onTracks } = props;

  const load = useCallback((): void => {
    notesClient.tracks(props.mediaId).then(
      (listed) => {
        setTracks(listed);
        setError(null);
        onTracks(listed);
      },
      (failure: unknown) => {
        setError(failure instanceof Error ? failure : new Error(String(failure)));
      },
    );
  }, [props.mediaId, onTracks]);

  useEffect(() => {
    load();
  }, [load]);

  const imported = (result: TrackImport): void => {
    setDialog(null);
    const language = result.lang === null ? t('captions.dialog.noLanguage') : languageName(result.lang, locale);
    // The route answers `replaced: false` both for a first import and for a
    // file that changed nothing, so the count before the import decides which
    // sentence is true.
    const before = (tracks ?? []).find((track) => track.lang === (result.lang ?? ''))?.cues ?? 0;
    setStatus(
      before === 0 || result.replaced
        ? result.cues === 1
          ? t('captions.importedOne', { language })
          : t('captions.imported', { language, cues: result.cues })
        : t('captions.unchanged', { language }),
    );
    load();
  };

  const remove = (track: TrackSummary): void => {
    if (busy) {
      return;
    }
    setBusy(true);
    notesClient.removeTrack(props.mediaId, track.lang).then(
      () => {
        setBusy(false);
        setRemoving(null);
        setStatus(
          t('captions.removed', {
            language: track.lang === '' ? t('captions.dialog.noLanguage') : languageName(track.lang, locale),
          }),
        );
        load();
      },
      (failure: unknown) => {
        setBusy(false);
        setError(failure instanceof Error ? failure : new Error(String(failure)));
      },
    );
  };

  const dialogFor = dialog === null ? null : (
    <CaptionDialog
      mediaId={props.mediaId}
      mode={dialog}
      existing={(tracks ?? []).map((track) => track.lang).filter((lang) => lang !== '')}
      defaultLang={dialog.replace?.lang ?? props.mediaLang}
      file={null}
      onClose={() => {
        setDialog(null);
      }}
      onImported={imported}
    />
  );

  return (
    <section className="captions" aria-label={t('captions.heading')}>
      <h3>{t('captions.heading')}</h3>

      {error === null ? null : (
        <p className="note-field-error" role="alert">
          {t('captions.loadFailed')}
        </p>
      )}

      {tracks === null ? null : tracks.length === 0 ? (
        <p className="captions-empty">{t('captions.empty')}</p>
      ) : (
        <ul className="caption-rows">
          {tracks.map((track) => {
            const language =
              track.lang === '' ? t('captions.dialog.noLanguage') : languageName(track.lang, locale);
            const when = track.imported === null ? null : formatDateTime(track.imported, locale);
            return (
              <li key={track.lang} className="caption-row">
                <p>
                  <strong>{language}</strong>
                  {': '}
                  {rowText(track, when, t)}
                </p>
                {track.handEdited === 0 ? null : (
                  <p className="caption-hand">
                    {t('captions.handEdited', { count: String(track.handEdited) })}
                  </p>
                )}
                {props.writable ? (
                  <div className="caption-actions">
                    <button
                      type="button"
                      onClick={() => {
                        setStatus(null);
                        setDialog({ replace: track });
                      }}
                    >
                      {t('captions.replace')}
                    </button>
                    {track.lang === '' ? null : (
                      <a href={captionsUrl(props.mediaId, track.lang)} download>
                        {t('captions.download')}
                      </a>
                    )}
                    {removing !== null && removing.lang === track.lang ? (
                      <span className="caption-remove-confirm" role="group" aria-label={t('captions.remove')}>
                        <span>
                          {t('captions.removeConfirm', { cues: track.cues, language })}
                        </span>{' '}
                        <button type="button" onClick={() => { remove(track); }}>
                          {t('captions.removeDo', { language })}
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setRemoving(null);
                          }}
                        >
                          {t('captions.keep')}
                        </button>
                      </span>
                    ) : (
                      <button
                        type="button"
                        onClick={() => {
                          setStatus(null);
                          setRemoving(track);
                        }}
                      >
                        {t('captions.remove')}
                      </button>
                    )}
                  </div>
                ) : null}
              </li>
            );
          })}
        </ul>
      )}

      {props.writable ? (
        <button
          type="button"
          className="caption-add"
          onClick={() => {
            setStatus(null);
            setDialog({ replace: null });
          }}
        >
          {tracks === null || tracks.length === 0 ? t('captions.add') : t('captions.addOther')}
        </button>
      ) : null}

      {status === null ? null : (
        <p className="caption-status" role="status">
          {status}
        </p>
      )}

      {dialogFor}
    </section>
  );
};

export interface UseAsCaptionsProps {
  /** The caption media itself: its file, its language, its item. */
  readonly media: Media;
}

/**
 * A caption file attached to an item, given to one of the item's recordings.
 * The admin fetches the file's bytes and posts them where the dialog posts
 * them, so the server reads them exactly as it reads an upload.
 */
export const UseAsCaptions = ({ media }: UseAsCaptionsProps): ReactElement => {
  const translator = useTranslator(annotations);
  const { t, locale } = translator;
  const [targets, setTargets] = useState<readonly Media[] | null>(null);
  const [picked, setPicked] = useState<Media | null>(null);
  const [existing, setExisting] = useState<readonly string[]>([]);
  const [file, setFile] = useState<File | null>(null);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const itemId = media.item?.id ?? null;
  useEffect(() => {
    if (itemId === null) {
      setTargets([]);
      return;
    }
    let live = true;
    api.listMediaForItem(itemId, { romeka_kind: ['audio', 'video'], perPage: 100 }).then(
      (page) => {
        if (live) {
          setTargets(page.items.filter((entry) => entry.id !== media.id));
        }
      },
      (failure: unknown) => {
        if (live) {
          setError(failure instanceof Error ? failure : new Error(String(failure)));
        }
      },
    );
    return () => {
      live = false;
    };
  }, [itemId, media.id]);

  const pick = (target: Media): void => {
    const url = media.originalUrl;
    if (url === null) {
      setError(new Error(t('captions.useAsNoFile')));
      return;
    }
    setReading(true);
    setError(null);
    Promise.all([fetch(url), notesClient.tracks(target.id).catch(() => [])])
      .then(async ([response, tracks]) => {
        if (!response.ok) {
          throw new Error(`HTTP ${String(response.status)}`);
        }
        const blob = await response.blob();
        const name = mediaName(media) ?? 'captions.vtt';
        setFile(new File([blob], name, { type: media.mediaType ?? blob.type }));
        setExisting(tracks.map((track) => track.lang).filter((lang) => lang !== ''));
        setPicked(target);
      })
      .catch((failure: unknown) => {
        setError(failure instanceof Error ? failure : new Error(String(failure)));
      })
      .finally(() => {
        setReading(false);
      });
  };

  return (
    <section className="captions" aria-label={t('captions.heading')}>
      <h3>{t('captions.useAsHeading')}</h3>
      {itemId === null ? (
        <p className="captions-empty">{t('captions.useAsNoItem')}</p>
      ) : targets === null ? null : targets.length === 0 ? (
        <p className="captions-empty">{t('captions.useAsNoTargets')}</p>
      ) : (
        <>
          <p>{t('captions.useAsPick')}</p>
          <ul className="caption-targets">
            {targets.map((target) => (
              <li key={target.id}>
                <button
                  type="button"
                  onClick={() => {
                    pick(target);
                  }}
                >
                  {mediaName(target) ?? t('captions.useAsUntitled')}
                  <span className="caption-target-kind">
                    {t(mediaKind(target) === 'video' ? 'captions.useAsVideo' : 'captions.useAsAudio')}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      {reading ? <p role="status">{t('captions.useAsReading')}</p> : null}
      {error === null ? null : (
        <p className="note-field-error" role="alert">
          {error.message}
        </p>
      )}
      {status === null ? null : (
        <p className="caption-status" role="status">
          {status}
        </p>
      )}
      {picked === null || file === null ? null : (
        <CaptionDialog
          mediaId={picked.id}
          mode={{ replace: null }}
          existing={existing}
          defaultLang={media.lang}
          file={file}
          onClose={() => {
            setPicked(null);
            setFile(null);
          }}
          onImported={(result) => {
            const language =
              result.lang === null ? t('captions.dialog.noLanguage') : languageName(result.lang, locale);
            setStatus(
              result.cues === 1
                ? t('captions.importedOne', { language })
                : t('captions.imported', { language, cues: result.cues }),
            );
            setPicked(null);
            setFile(null);
          }}
        />
      )}
      {/* The language list of the chosen recording arrives with the dialog's own read. */}
    </section>
  );
};
