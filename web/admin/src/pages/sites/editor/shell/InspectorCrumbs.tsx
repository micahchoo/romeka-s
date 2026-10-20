/**
 * The inspector's crumb, "Page › <selection>": its Page button clears the
 * selection, so the inspector shows the page settings again.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import { editor } from '../../../../i18n/namespaces/editor';

export const InspectorCrumbs = ({
  label,
  onPage,
}: {
  readonly label: string;
  readonly onPage: () => void;
}): ReactElement => {
  const { t } = useTranslator(editor);
  return (
    <nav className="inspector-crumbs" aria-label={t('editor.crumbs')}>
      <button type="button" title={t('editor.crumbPageHint')} onClick={onPage}>
        {t('editor.crumbPage')}
      </button>
      <span aria-hidden="true">›</span>
      <span aria-current="location">{label}</span>
    </nav>
  );
};
