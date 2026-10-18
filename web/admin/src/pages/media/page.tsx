import type { ReactElement } from 'react';
import { mediaIdFromParam } from '../../api/parse';
import type { PageProps } from '../../app/router';
import { useTranslator } from '../../i18n';
import { MediaDetail } from './detail';
import { MediaList } from './list';

/** `#/media` lists; `#/media/<id>` shows one media. */
export const MediaPage = ({ param }: PageProps): ReactElement => {
  const { t } = useTranslator();
  if (param === null) {
    return <MediaList />;
  }
  const mediaId = mediaIdFromParam(param);
  if (mediaId === null) {
    return <p role="alert">{t('route.invalidMediaId', { param })}</p>;
  }
  return <MediaDetail key={String(mediaId)} mediaId={mediaId} />;
};
