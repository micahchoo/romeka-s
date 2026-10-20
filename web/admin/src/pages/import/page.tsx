/**
 * `#/import` is the start page and the past imports; `#/import/<id>` opens one
 * import where it stands, and `#/import/<id>/<mode>/<screen>` one screen of it
 * (`screens.ts`).
 */
import type { ReactElement } from 'react';
import { brandId } from '../../api/parse';
import type { PageProps } from '../../app/router';
import { DraftShell } from './draft-shell';
import { ImportHome } from './home';
import './import.css';
import { useImportText } from './messages';
import { placeFromSegments } from './screens';

export const ImportPage = ({ param, rest = [] }: PageProps): ReactElement => {
  const { t } = useImportText();
  if (param === null) {
    return <ImportHome />;
  }
  if (!/^[1-9][0-9]{0,14}$/.test(param)) {
    return <p role="alert">{t('shell.badId', { param })}</p>;
  }
  const id = brandId<'ImportId'>(Number(param));
  const place = placeFromSegments(rest.slice(1));
  return <DraftShell key={param} id={id} place={place} />;
};
