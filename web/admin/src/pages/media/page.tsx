import type { ReactElement } from 'react';
import { mediaIdFromParam } from '../../api/media';
import type { PageProps } from '../../app/router';
import { useTranslator } from '../../i18n';
import { MediaDetail } from './detail';
import { MediaList } from './list';
import { resources } from '../../i18n/namespaces/resources';

/** `#/media` lists; `#/media/<id>` shows one media. */
export const MediaPage = ({ param }: PageProps): ReactElement => {
  const { t } = useTranslator(resources);
  if (param === null) {
    return <MediaList />;
  }
  const mediaId = mediaIdFromParam(param);
  if (mediaId === null) {
    return <p role="alert">{t('route.invalidMediaId', { param })}</p>;
  }
  return <MediaDetail key={String(mediaId)} mediaId={mediaId} />;
};
