/**
 * The media stage: the one place an editor sees or plays a media's own file.
 *
 * The stage picks one browser control per kind (`view.ts`), so an audio file is
 * a player and a PDF is its first page, never a broken picture. It loads as its
 * own chunk because a route that lists media does not need the players, and it
 * is one component with one job, because the notes panel (A-3) mounts beside it
 * and owns everything about notes.
 */
import type { ReactElement, RefCallback } from 'react';
import { useEffect, useState } from 'react';
import type { Media } from '../../../api/types';
import { defaultBaseUrl } from '../../../api/client';
import { formatBytes } from '../../../app/format';
import { languageName } from '../../../app/language';
import { KIND_LABEL } from '../../../app/media';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { useTranslator } from '../../../i18n';
import { resources } from '../../../i18n/namespaces/resources';
import { recallPosition, rememberPosition } from './play-position';
import { EmbedCard, ManifestCard, ThirdPartyCard } from './third-party';
import { stageView } from './view';
import type { FileFacts, StageView } from './view';
import './stage.css';

/** The keys this stage reads: the shell catalogue and the media namespace. */
type StageTranslator = Translator<MessageKey | KeyOf<typeof resources>>;

/** The caption languages the player offers, as the panel reports them. */
export interface CaptionTrack {
  readonly lang: string;
  readonly cues: number;
}

/** The `<track>` elements of a player: one per language with cues (A-6). */
const CaptionTracks = ({
  tracks,
  mediaId,
  mediaLang,
  translator,
}: {
  readonly tracks: readonly CaptionTrack[];
  readonly mediaId: number;
  readonly mediaLang: string | null;
  readonly translator: StageTranslator;
}): ReactElement => (
  <>
    {tracks
      .filter((track) => track.lang !== '' && track.cues > 0)
      .map((track) => (
        <track
          key={track.lang}
          kind="captions"
          src={`${defaultBaseUrl()}/annotations/media/${String(mediaId)}/captions.vtt?lang=${encodeURIComponent(track.lang)}`}
          srcLang={track.lang}
          label={languageName(track.lang, translator.locale)}
          {...(mediaLang === track.lang ? { default: true } : {})}
        />
      ))}
  </>
);

/** Where a player left off, unless the kept time is past the end. */
const resumeAt = (player: HTMLMediaElement, mediaId: number): void => {
  const kept = recallPosition(mediaId);
  if (kept === null || kept <= 0) {
    return;
  }
  const { duration } = player;
  if (Number.isFinite(duration) && kept >= duration - 0.5) {
    return;
  }
  player.currentTime = kept;
};

const FileFactsLine = ({
  file,
  translator,
}: {
  readonly file: FileFacts;
  readonly translator: StageTranslator;
}): ReactElement => {
  const label = translator.t(KIND_LABEL[file.kind]);
  // A PDF's extension repeats the kind's own name, so it earns no second line.
  const extension =
    file.extension !== null && file.extension.toLowerCase() !== label.toLowerCase()
      ? file.extension
      : null;
  return (
    <p className="media-stage-file-facts">
      <span>{label}</span>
      {extension === null ? null : <span>{extension}</span>}
      {file.size === null ? null : <span>{formatBytes(file.size, translator)}</span>}
    </p>
  );
};

/** A file the stage cannot show: its name, what it is, and the way to take it. */
const FileCard = ({
  file,
  translator,
  note,
}: {
  readonly file: FileFacts;
  readonly translator: StageTranslator;
  readonly note: string | null;
}): ReactElement => {
  const { t } = translator;
  return (
    <div className="media-stage-file">
      {note === null ? null : <p className="media-stage-file-note">{note}</p>}
      {file.name === null ? null : <p className="media-stage-file-name">{file.name}</p>}
      {file.url === null ? null : (
        <a className="button-link" href={file.url}>
          {t('media.stage.downloadFile')}
        </a>
      )}
      {file.url === null && file.sourceUrl !== null ? (
        <a className="button-link" href={file.sourceUrl} target="_blank" rel="noreferrer">
          {t('media.stage.openSource')}
        </a>
      ) : null}
    </div>
  );
};

const ImageStage = ({
  view,
  translator,
}: {
  readonly view: Extract<StageView, { element: 'image' }>;
  readonly translator: StageTranslator;
}): ReactElement => {
  if (view.url === null) {
    const note =
      view.file.url === null && view.file.sourceUrl === null
        ? translator.t('media.stage.noFile')
        : translator.t('media.stage.noPreview');
    return <FileCard file={view.file} translator={translator} note={note} />;
  }
  return (
    <figure className="media-stage-figure">
      <img
        className={`media-stage-image${view.svg ? ' is-svg' : ''}`}
        src={view.url}
        alt={view.alt}
      />
    </figure>
  );
};

const AudioStage = ({
  view,
  mediaId,
  player,
  tracks,
  mediaLang,
  translator,
}: {
  readonly view: Extract<StageView, { element: 'audio' }>;
  readonly mediaId: number;
  readonly player: RefCallback<HTMLMediaElement> | undefined;
  readonly tracks: readonly CaptionTrack[];
  readonly mediaLang: string | null;
  readonly translator: StageTranslator;
}): ReactElement => (
  <audio
    className="media-stage-player"
    ref={player}
    controls
    preload="metadata"
    src={view.url}
    onLoadedMetadata={(event) => {
      resumeAt(event.currentTarget, mediaId);
    }}
    onTimeUpdate={(event) => {
      rememberPosition(mediaId, event.currentTarget.currentTime);
    }}
  >
    <CaptionTracks tracks={tracks} mediaId={mediaId} mediaLang={mediaLang} translator={translator} />
    <p>
      {translator.t('media.stage.playFailed')}{' '}
      <a href={view.url}>{translator.t('media.stage.downloadFile')}</a>
    </p>
  </audio>
);

const VideoStage = ({
  view,
  mediaId,
  player,
  tracks,
  mediaLang,
  translator,
}: {
  readonly view: Extract<StageView, { element: 'video' }>;
  readonly mediaId: number;
  readonly player: RefCallback<HTMLMediaElement> | undefined;
  readonly tracks: readonly CaptionTrack[];
  readonly mediaLang: string | null;
  readonly translator: StageTranslator;
}): ReactElement => (
  <video
    className="media-stage-player"
    ref={player}
    controls
    preload="metadata"
    src={view.url}
    {...(view.poster === null ? {} : { poster: view.poster })}
    onLoadedMetadata={(event) => {
      resumeAt(event.currentTarget, mediaId);
    }}
    onTimeUpdate={(event) => {
      rememberPosition(mediaId, event.currentTarget.currentTime);
    }}
  >
    <CaptionTracks tracks={tracks} mediaId={mediaId} mediaLang={mediaLang} translator={translator} />
    <p>
      {translator.t('media.stage.playFailed')}{' '}
      <a href={view.url}>{translator.t('media.stage.downloadFile')}</a>
    </p>
  </video>
);

/** The first page, and the browser's own PDF viewer only when asked. */
const PdfStage = ({
  view,
  translator,
}: {
  readonly view: Extract<StageView, { element: 'pdf' }>;
  readonly translator: StageTranslator;
}): ReactElement => {
  const [reading, setReading] = useState(false);
  const { t } = translator;
  const name = view.file.name ?? '';
  const open = (
    <a href={view.fileUrl} target="_blank" rel="noreferrer">
      {t('media.stage.pdfOpen')}
    </a>
  );
  return (
    <div className="media-stage-pdf">
      {reading ? (
        <object
          className="media-stage-pdf-object"
          type="application/pdf"
          data={view.fileUrl}
          aria-label={name === '' ? t('media.stage.pdfOpen') : name}
        >
          {view.pageUrl === null ? null : (
            <img className="media-stage-image" src={view.pageUrl} alt={name} />
          )}
          <p>
            {t('media.stage.pdfFallback')} {open}
          </p>
        </object>
      ) : (
        <>
          {view.pageUrl === null ? null : (
            <img className="media-stage-image" src={view.pageUrl} alt={name} />
          )}
          <button
            type="button"
            onClick={() => {
              setReading(true);
            }}
          >
            {t('media.stage.pdfRead')}
          </button>
        </>
      )}
      <p>{open}</p>
    </div>
  );
};

/** The first lines of a caption file, as text. The cues are the server's. */
const TextStage = ({
  view,
  translator,
}: {
  readonly view: Extract<StageView, { element: 'text' }>;
  readonly translator: StageTranslator;
}): ReactElement => {
  const [body, setBody] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let live = true;
    setBody(null);
    setFailed(false);
    const read = async (): Promise<string> => {
      const response = await fetch(view.textUrl);
      if (!response.ok) {
        throw new Error(`HTTP ${String(response.status)}`);
      }
      return response.text();
    };
    void read()
      .then((text) => {
        if (live) {
          setBody(firstLines(text, 40));
        }
      })
      .catch(() => {
        if (live) {
          setFailed(true);
        }
      });
    return () => {
      live = false;
    };
  }, [view.textUrl]);
  if (failed) {
    return (
      <div className="media-stage-file">
        <p className="media-stage-file-note">{translator.t('media.stage.textFailed')}</p>
        <a className="button-link" href={view.textUrl}>
          {translator.t('media.stage.downloadFile')}
        </a>
      </div>
    );
  }
  if (body === null) {
    return <p role="status">{translator.t('media.stage.textLoading')}</p>;
  }
  return (
    <pre className="media-stage-text" tabIndex={0}>
      {body}
    </pre>
  );
};

/**
 * The first `limit` lines of a file. A single corrupt line is cut at `LINE_MAX`
 * so one file cannot stretch the page.
 */
const LINE_MAX = 300;

const firstLines = (text: string, limit: number): string =>
  text
    .split(/\r\n?|\n/)
    .slice(0, limit)
    .map((line) => line.slice(0, LINE_MAX))
    .join('\n');

const StageBody = ({
  view,
  media,
  player,
  tracks,
  translator,
}: {
  readonly view: StageView;
  readonly media: Media;
  readonly player: RefCallback<HTMLMediaElement> | undefined;
  readonly tracks: readonly CaptionTrack[];
  readonly translator: StageTranslator;
}): ReactElement => {
  const mediaId = media.id;
  const mediaLang = media.lang;
  switch (view.element) {
    case 'image':
      return <ImageStage view={view} translator={translator} />;
    case 'audio':
      return (
        <AudioStage
          view={view}
          mediaId={mediaId}
          player={player}
          tracks={tracks}
          mediaLang={mediaLang}
          translator={translator}
        />
      );
    case 'video':
      return (
        <VideoStage
          view={view}
          mediaId={mediaId}
          player={player}
          tracks={tracks}
          mediaLang={mediaLang}
          translator={translator}
        />
      );
    case 'pdf':
      return <PdfStage view={view} translator={translator} />;
    case 'text':
      return <TextStage view={view} translator={translator} />;
    case 'outside':
      return <ThirdPartyCard url={view.url} host={view.host} />;
    case 'frame':
      return <EmbedCard mediaId={mediaId} url={view.url} host={view.host} />;
    case 'manifest':
      return <ManifestCard url={view.url} host={view.host} viewerUrl={view.viewerUrl} />;
    case 'file':
      return <FileCard file={view.file} translator={translator} note={null} />;
    case 'none':
      return <p className="media-stage-empty">{translator.t('media.stage.noFile')}</p>;
  }
};

/**
 * One media's stage, with the facts line the server can state about it. The
 * page hands in a ref callback and keeps the element it names, so the notes
 * panel can seek the player the editor is watching.
 */
export const MediaStage = ({
  media,
  player,
  tracks,
}: {
  readonly media: Media;
  readonly player?: RefCallback<HTMLMediaElement>;
  readonly tracks?: readonly CaptionTrack[];
}): ReactElement => {
  const translator = useTranslator(resources);
  const view = stageView(media);
  return (
    <div className="media-stage" data-stage={view.element}>
      <StageBody
        view={view}
        media={media}
        player={player}
        tracks={tracks ?? []}
        translator={translator}
      />
      {view.element === 'none' ? null : (
        <FileFactsLine file={view.file} translator={translator} />
      )}
    </div>
  );
};
