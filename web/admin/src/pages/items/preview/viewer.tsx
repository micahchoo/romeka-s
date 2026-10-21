/**
 * One file of the item, over the item page: the same stage the media page
 * shows, a native modal `<dialog>` around it, and the item's files in order as
 * Previous and Next.
 *
 * The file is URL state (`preview/url.ts`), so a shareable address opens it and
 * Back closes it. The viewer never renders a note: Annotate leaves for the
 * media page, which owns the panel.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useRef, useState } from 'react';
import type { Media, MediaId } from '../../../api/types';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import type { TrackSummary } from '../../media/annotations/client';
import { notesClient } from '../../media/annotations/client';
import { annotateHref, mediaHref } from '../../media/annotations/link';
import { mediaName } from '../../../app/media';
import { stageView } from '../../media/stage/view';
import './viewer.css';

/** The stage is the media page's chunk, so the viewer fetches it rather than copies it. */
const MediaStage = lazy(() =>
  import('../../media/stage/stage').then((module) => ({ default: module.MediaStage })),
);

/** A control the stage owns: its own keys are the browser's, not the viewer's. */
const isMediaControl = (target: EventTarget | null): boolean =>
  target instanceof HTMLMediaElement || target instanceof HTMLObjectElement;

export const MediaViewer = ({
  files,
  mediaId,
  writable,
  onShow,
  onClose,
}: {
  /** The item's files in item order. */
  readonly files: readonly Media[];
  readonly mediaId: MediaId;
  /** Whether this file's notes may be written, or null while it is unknown. */
  readonly writable: boolean | null;
  /** Show another file without leaving the viewer. */
  readonly onShow: (id: MediaId) => void;
  readonly onClose: () => void;
}): ReactElement | null => {
  const { t } = useTranslator(annotations, common);
  const dialog = useRef<HTMLDialogElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  const index = files.findIndex((entry) => entry.id === mediaId);
  const media = index === -1 ? undefined : files[index];
  const [tracks, setTracks] = useState<readonly TrackSummary[]>([]);
  /** A runtime without `showModal` (the tests' jsdom) draws the same markup inline. */
  const [inline, setInline] = useState(false);
  const kind = media === undefined ? 'none' : stageView(media).element;

  useEffect(() => {
    const element = dialog.current;
    if (element === null) {
      return;
    }
    if (typeof element.showModal === 'function') {
      try {
        if (!element.open) {
          element.showModal();
        }
        close.current?.focus();
        return;
      } catch {
        // No top layer after all: the inline form below still works.
      }
    }
    setInline(true);
  }, []);

  // The captions a recording has, from the same route the media page reads.
  useEffect(() => {
    setTracks([]);
    if (media === undefined || (kind !== 'audio' && kind !== 'video')) {
      return;
    }
    let live = true;
    notesClient.tracks(media.id).then(
      (answer) => {
        if (live) {
          setTracks(answer);
        }
      },
      () => {
        // A recording without captions still plays.
      },
    );
    return () => {
      live = false;
    };
  }, [media, kind]);

  if (media === undefined) {
    return null;
  }

  const go = (step: number): void => {
    const next = files[index + step];
    if (next !== undefined) {
      onShow(next.id);
    }
  };

  return (
    <dialog
      ref={dialog}
      {...(inline ? { open: true } : {})}
      className="media-viewer"
      aria-labelledby="media-viewer-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          onClose();
          return;
        }
        if (isMediaControl(event.target)) {
          return;
        }
        if (event.key === 'ArrowLeft') {
          event.preventDefault();
          go(-1);
        } else if (event.key === 'ArrowRight') {
          event.preventDefault();
          go(1);
        }
      }}
    >
      <header className="media-viewer-head">
        <div>
          <h2 id="media-viewer-title">{mediaName(media) ?? t('common.untitled')}</h2>
          <p className="media-viewer-position">
            {t('preview.position', { index: index + 1, total: files.length })}
          </p>
        </div>
        <button
          type="button"
          className="media-viewer-close"
          ref={close}
          autoFocus
          aria-label={t('preview.close')}
          onClick={onClose}
        >
          ×
        </button>
      </header>

      <div className="media-viewer-stage">
        <Suspense fallback={<p role="status">{t('preview.loading')}</p>}>
          <MediaStage media={media} tracks={tracks} />
        </Suspense>
      </div>

      <footer className="media-viewer-actions">
        <div className="media-viewer-paging">
          <button type="button" disabled={index <= 0} onClick={() => {
              go(-1);
            }}>
            {t('preview.previous')}
          </button>
          <button
            type="button"
            disabled={index >= files.length - 1}
            onClick={() => {
              go(1);
            }}
          >
            {t('preview.next')}
          </button>
        </div>
        <div className="media-viewer-links">
          {writable === true ? (
            <a className="button-link" href={annotateHref(media.id)}>
              {t('notes.cardAnnotate')}
            </a>
          ) : null}
          <a href={mediaHref(media.id)}>{t('preview.openPage')}</a>
        </div>
      </footer>
    </dialog>
  );
};
