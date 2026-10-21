/**
 * An outside page on the media stage: a card that names the host it will
 * contact, and a link that leaves the admin.
 *
 * Nothing here loads third-party markup. An oEmbed or HTML media is rendered by
 * the server in a sandboxed frame once that route exists (the S3 unit); until
 * then the card is the whole stage, because the admin must not decide what
 * provider HTML is safe to run.
 */
import type { ReactElement } from 'react';
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
