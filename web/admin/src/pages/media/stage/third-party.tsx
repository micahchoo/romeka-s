/**
 * An outside page on the media stage: a card that names the host it will
 * contact, and a link that leaves the admin.
 *
 * Nothing here loads third-party markup before the editor asks for it. An
 * oEmbed, HTML or YouTube media is shown from the server's answer to
 * `GET /api/romeka/media/{id}/player`: the provider's own player URL, which the
 * card frames directly, or the server's render in a sandboxed frame
 * (`GET /api/romeka/media/{id}/render`), because the admin must not decide
 * what provider HTML is safe to run. The frame loads that route as its `src`
 * rather than through `srcdoc`: the response's own CSP and `X-Frame-Options`
 * then govern the frame, and a private media's cookie travels with the request
 * without the body passing through this page.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api, defaultBaseUrl } from '../../../api/client';
import type { MediaId } from '../../../api/types';
import { useTranslator } from '../../../i18n';
import { resources } from '../../../i18n/namespaces/resources';
import type { EmbedFallback } from './view';
import { hostOf } from './view';

export const ThirdPartyCard = ({
  url,
  host,
}: {
  readonly url: string;
  readonly host: string;
}): ReactElement => {
  const { t } = useTranslator(resources);
  return (
    <div className="media-stage-outside">
      <p>{t('media.stage.embedHeading', { host })}</p>
      <a className="button-link" href={url} target="_blank" rel="noreferrer">
        {t('media.stage.embedOpen', { host })}
      </a>
    </div>
  );
};

/**
 * What the card has loaded, or what it is asking the server for. The server
 * answers whether the media has a provider player, so the card never reads
 * provider markup itself. `link` is a media the server names no player for and
 * whose render the server refuses: the card stays the link-out it was, because
 * the 415 document in a frame would say less than the link.
 */
type EmbedFrame =
  | { readonly state: 'closed' }
  | { readonly state: 'asking' }
  | { readonly state: 'strict' }
  | { readonly state: 'player'; readonly url: string }
  | { readonly state: 'link' };

/**
 * The line above the frame, which says what is true of the stage now: nothing
 * has loaded, the provider is playing, or the server's render is open.
 */
const EmbedHeading = ({
  frame,
  host,
}: {
  readonly frame: EmbedFrame;
  readonly host: string | null;
}): ReactElement => {
  const { t } = useTranslator(resources);
  if (frame.state === 'player') {
    return <p>{t('media.stage.embedPlaying', { host: hostOf(frame.url) })}</p>;
  }
  if (frame.state === 'strict') {
    return (
      <p>
        {host === null
          ? t('media.stage.embedSnippetFrame')
          : t('media.stage.embedFrame', { host })}
      </p>
    );
  }
  return (
    <p>
      {host === null
        ? t('media.stage.embedSnippetHeading')
        : t('media.stage.embedHeading', { host })}
    </p>
  );
};

/**
 * An oEmbed, HTML or YouTube media: a facade that names the host, then a frame.
 * The server decides which frame. When it names a player, `/player` answers
 * that URL and the card frames the provider directly: `allow-same-origin`
 * beside `allow-scripts` then grants the provider's own origin, never the
 * admin's, and without it a nested provider player inherits an opaque origin
 * and refuses to draw or play. Every media the server names no player for keeps
 * the strict frame of its render, which runs the provider's script and presents
 * full screen and never grants `allow-same-origin` beside `allow-scripts`; a
 * media whose render the server refuses keeps the card and its link instead.
 */
export const EmbedCard = ({
  mediaId,
  url,
  host,
  fallback,
}: {
  readonly mediaId: MediaId;
  readonly url: string | null;
  readonly host: string | null;
  readonly fallback: EmbedFallback;
}): ReactElement => {
  const { t } = useTranslator(resources);
  const [frame, setFrame] = useState<EmbedFrame>({ state: 'closed' });
  const title =
    host === null
      ? t('media.stage.embedSnippetFrame')
      : t('media.stage.embedFrame', { host });
  const missed = (): EmbedFrame => (fallback === 'render' ? { state: 'strict' } : { state: 'link' });
  const load = (): void => {
    setFrame({ state: 'asking' });
    // A refused or failed answer is what this card showed before the player
    // route existed: the render frame, or the link alone.
    api.getMediaPlayer(mediaId).then(
      (player) => {
        setFrame(player === null ? missed() : { state: 'player', url: player });
      },
      () => {
        setFrame(missed());
      },
    );
  };
  return (
    <div className="media-stage-outside">
      <EmbedHeading frame={frame} host={host} />
      {frame.state === 'closed' || frame.state === 'asking' ? (
        <button type="button" disabled={frame.state === 'asking'} onClick={load}>
          {t('media.stage.embedLoad')}
        </button>
      ) : null}
      {frame.state === 'player' ? (
        <iframe
          className="media-stage-frame"
          sandbox="allow-scripts allow-same-origin allow-presentation allow-popups"
          allow="fullscreen; picture-in-picture"
          referrerPolicy="strict-origin-when-cross-origin"
          src={frame.url}
          title={title}
        />
      ) : frame.state === 'strict' ? (
        <iframe
          className="media-stage-frame"
          sandbox="allow-scripts allow-presentation"
          src={`${defaultBaseUrl()}/api/romeka/media/${String(mediaId)}/render`}
          title={title}
        />
      ) : null}
      {url === null || host === null ? null : (
        <a className="button-link" href={url} target="_blank" rel="noreferrer">
          {t('media.stage.embedOpen', { host })}
        </a>
      )}
    </div>
  );
};

/** A IIIF manifest: opened in the viewer the server serves, or read as JSON. */
export const ManifestCard = ({
  url,
  host,
  viewerUrl,
}: {
  readonly url: string;
  readonly host: string;
  readonly viewerUrl: string;
}): ReactElement => {
  const { t } = useTranslator(resources);
  return (
    <div className="media-stage-outside">
      <p>{t('media.stage.manifestHeading', { host })}</p>
      <a className="button-link" href={viewerUrl} target="_blank" rel="noreferrer">
        {t('media.stage.manifestOpen')}
      </a>
      <a href={url} target="_blank" rel="noreferrer">
        {t('media.stage.manifestSource')}
      </a>
    </div>
  );
};
