/**
 * An outside page on the media stage: a card that names the host it will
 * contact, and a link that leaves the admin.
 *
 * Nothing here loads third-party markup before the editor asks for it. An
 * oEmbed or HTML media is shown from the server's own render in a sandboxed
 * frame (`GET /api/romeka/media/{id}/render`), because the admin must not decide
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
 * answers whether the render is one provider player, so the card never reads
 * provider markup itself.
 */
type EmbedFrame =
  | { readonly state: 'closed' }
  | { readonly state: 'asking' }
  | { readonly state: 'strict' }
  | { readonly state: 'player'; readonly url: string };

/**
 * An oEmbed or HTML media: a facade that names the host, then a frame. The
 * server decides which frame. When its render of the media is one allow-listed
 * provider frame, `/player` answers that URL and the card frames the provider
 * directly: `allow-same-origin` beside `allow-scripts` then grants the
 * provider's own origin, never the admin's, and without it a nested provider
 * player inherits an opaque origin and refuses to draw or play. Every other
 * render keeps the strict frame, which runs the provider's script and presents
 * full screen and never grants `allow-same-origin` beside `allow-scripts`.
 */
export const EmbedCard = ({
  mediaId,
  url,
  host,
}: {
  readonly mediaId: MediaId;
  readonly url: string | null;
  readonly host: string | null;
}): ReactElement => {
  const { t } = useTranslator(resources);
  const [frame, setFrame] = useState<EmbedFrame>({ state: 'closed' });
  const title =
    host === null
      ? t('media.stage.embedSnippetFrame')
      : t('media.stage.embedFrame', { host });
  const load = (): void => {
    setFrame({ state: 'asking' });
    // A refused or failed answer is the strict frame, which is what this card
    // showed before the player route existed.
    api.getMediaPlayer(mediaId).then(
      (player) => {
        setFrame(player === null ? { state: 'strict' } : { state: 'player', url: player });
      },
      () => {
        setFrame({ state: 'strict' });
      },
    );
  };
  return (
    <div className="media-stage-outside">
      <p>
        {host === null
          ? t('media.stage.embedSnippetHeading')
          : t('media.stage.embedHeading', { host })}
      </p>
      {frame.state === 'closed' || frame.state === 'asking' ? (
        <button type="button" disabled={frame.state === 'asking'} onClick={load}>
          {t('media.stage.embedLoad')}
        </button>
      ) : frame.state === 'player' ? (
        <iframe
          className="media-stage-frame"
          sandbox="allow-scripts allow-same-origin allow-presentation allow-popups"
          allow="fullscreen; picture-in-picture"
          referrerPolicy="strict-origin-when-cross-origin"
          src={frame.url}
          title={title}
        />
      ) : (
        <iframe
          className="media-stage-frame"
          sandbox="allow-scripts allow-presentation"
          src={`${defaultBaseUrl()}/api/romeka/media/${String(mediaId)}/render`}
          title={title}
        />
      )}
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
