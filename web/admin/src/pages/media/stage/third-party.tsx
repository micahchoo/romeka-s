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
import { defaultBaseUrl } from '../../../api/client';
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
 * An oEmbed or HTML media: a facade that names the host, then the server's
 * render in a frame. The sandbox runs the provider's own script and lets it
 * present full screen; it never grants `allow-same-origin` beside
 * `allow-scripts`, which would let the frame script the admin.
 */
export const EmbedCard = ({
  mediaId,
  url,
  host,
}: {
  readonly mediaId: number;
  readonly url: string | null;
  readonly host: string | null;
}): ReactElement => {
  const { t } = useTranslator(resources);
  const [shown, setShown] = useState(false);
  return (
    <div className="media-stage-outside">
      <p>
        {host === null
          ? t('media.stage.embedSnippetHeading')
          : t('media.stage.embedHeading', { host })}
      </p>
      {shown ? (
        <iframe
          className="media-stage-frame"
          sandbox="allow-scripts allow-presentation"
          src={`${defaultBaseUrl()}/api/romeka/media/${String(mediaId)}/render`}
          title={
            host === null
              ? t('media.stage.embedSnippetFrame')
              : t('media.stage.embedFrame', { host })
          }
        />
      ) : (
        <button
          type="button"
          onClick={() => {
            setShown(true);
          }}
        >
          {t('media.stage.embedLoad')}
        </button>
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
