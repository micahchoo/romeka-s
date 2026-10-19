/**
 * The save-as-template dialog: a name, a description and the button that writes
 * the site's `romeka_page_templates` row. The shell loads it on demand, so it is
 * its own chunk and the editor does not carry it until the user opens it.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../../../api/client';
import { encodeSiteSettings } from '../../../../api/write';
import type { JsonValue, SiteId } from '../../../../api/types';
import { SaveStatus, TextField } from '../../../../app/fields';
import { useSave } from '../../../../app/save';
import { useTranslator } from '../../../../i18n';
import {
  SAVED_TEMPLATES_KEY,
  newTemplateId,
  savedTemplates,
  savedTemplatesWire,
  withSavedTemplates,
} from '../../templates';
import { toSavedTemplate } from '../wire';
import type { PageContent } from '../draft';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import { editor } from '../../../../i18n/namespaces/editor';

export interface SaveTemplateDialogProps {
  readonly siteId: SiteId;
  /** The site settings the editor loaded, so the write appends to what exists. */
  readonly siteSettings: JsonValue | null;
  readonly draft: PageContent;
  /** The site settings map after the write, so the editor's lists update. */
  readonly onSaved: (settings: JsonValue) => void;
  readonly onClose: () => void;
}

export const SaveTemplateDialog = ({
  siteId,
  siteSettings,
  draft,
  onSaved,
  onClose,
}: SaveTemplateDialogProps): ReactElement => {
  const { t } = useTranslator(common, sites, editor);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const { state: saveState, save } = useSave();
  const trimmed = name.trim();

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    if (trimmed === '') {
      return;
    }
    save(async () => {
      const template = toSavedTemplate(draft, newTemplateId(), trimmed, description.trim());
      const next = [...savedTemplates(siteSettings), template];
      await api.patchSite(
        siteId,
        encodeSiteSettings({ [SAVED_TEMPLATES_KEY]: savedTemplatesWire(next) }),
      );
      onSaved(withSavedTemplates(siteSettings, next));
    });
  };

  return (
    <div className="template-dialog-backdrop">
      <div
        className="template-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t('template.saveAs')}
      >
        <h2>{t('template.saveAs')}</h2>
        <form onSubmit={submit}>
          <TextField id="template-name" label={t('template.name')} value={name} onChange={setName} />
          <TextField
            id="template-description"
            label={t('template.description')}
            value={description}
            onChange={setDescription}
          />
          <SaveStatus state={saveState} />
          <div className="template-dialog-actions">
            <button type="submit" disabled={trimmed === '' || saveState.status === 'saving'}>
              {t('template.saveAs')}
            </button>
            <button type="button" onClick={onClose}>
              {t('action.cancel')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
