/**
 * `#/import` is the start and the history; `#/import/<id>/<step>` is one
 * import's steps (`draft.tsx`), and `#/import/<id>/grid/problems` the Grid
 * with only the rows that have problems.
 */
import type { ReactElement } from 'react';
import { importIdFromParam } from '../../api/imports';
import type { PageProps } from '../../app/router';
import { ImportDraft } from './draft';
import { ImportHome } from './home';
import './import.css';
import { useImportText } from './messages';
import { PROBLEMS_VIEW, tabFromSlug, tabHref } from './tabs';

export const ImportPage = ({ param, rest = [] }: PageProps): ReactElement => {
  const { t } = useImportText();
  if (param === null) {
    return <ImportHome />;
  }
  const id = importIdFromParam(param);
  if (id === null) {
    return <p role="alert">{t('archive.invalidId', { param })}</p>;
  }
  const tab = tabFromSlug(rest[1]);
  if (tab === null) {
    return (
      <p role="alert">
        <a href={tabHref(id, 'folder')}>{t('archive.invalidTab')}</a>
      </p>
    );
  }
  return <ImportDraft key={String(id)} id={id} tab={tab} problemsOnly={tab === 'grid' && rest[2] === PROBLEMS_VIEW} />;
};
