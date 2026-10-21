import type { ReactElement } from 'react';
import { mediaIdFromParam } from '../../api/parse';
import type { PageProps } from '../../app/router';
import { useHash } from '../../app/router';
import { useTranslator } from '../../i18n';
import { MediaDetail } from './detail';
import { MediaList } from './list';
import { notesFocusOf } from './annotations/link';
import { resources } from '../../i18n/namespaces/resources';

/**
 * `#/media` lists; `#/media/<id>` shows one media. The query asks the notes
 * panel for a job: `?notes=add` or `?note=<id>` (`annotations/link.ts`).
 */
export const MediaPage = ({ param }: PageProps): ReactElement => {
  const { t } = useTranslator(resources);
  const hash = useHash();
  if (param === null) {
    return <MediaList />;
  }
  const mediaId = mediaIdFromParam(param);
  if (mediaId === null) {
    return <p role="alert">{t('route.invalidMediaId', { param })}</p>;
  }
  return (
    <MediaDetail key={String(mediaId)} mediaId={mediaId} focus={notesFocusOf(hash)} />
  );
};
