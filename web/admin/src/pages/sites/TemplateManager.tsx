/**
 * The saved-template manager: rename or delete the templates this site stores
 * in its `romeka_page_templates` row. The settings page loads it on demand, so
 * it is its own chunk and the settings route does not carry it until the user
 * opens it.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { encodeSiteSettings } from '../../api/write';
import type { JsonValue, SiteId } from '../../api/types';
import { SaveStatus, TextField } from '../../app/fields';
import { useSave } from '../../app/save';
import { useTranslator } from '../../i18n';
import {
  SAVED_TEMPLATES_KEY,
  savedTemplates,
  savedTemplatesWire,
} from './templates';
import type { SavedTemplate } from './templates';
import { sites } from '../../i18n/namespaces/sites';

export interface TemplateManagerProps {
  readonly siteId: SiteId;
  /** The site settings the page loaded, which carry the saved templates. */
  readonly siteSettings: JsonValue | null;
  readonly onClose: () => void;
}

/** One saved template, with its rename fields and its delete button. */
const TemplateRow = ({
  template,
  onRename,
  onDelete,
}: {
  readonly template: SavedTemplate;
  readonly onRename: (name: string, description: string) => void;
  readonly onDelete: () => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const [name, setName] = useState(template.name);
  const [description, setDescription] = useState(template.description);
  const trimmed = name.trim();
  const changed = trimmed !== template.name || description.trim() !== template.description;

  return (
    <li className="template-manager-row">
      <TextField
        id={`template-${template.id}-name`}
        label={t('template.name')}
        value={name}
        onChange={setName}
      />
      <TextField
        id={`template-${template.id}-description`}
        label={t('template.description')}
        value={description}
        onChange={setDescription}
      />
      <button
        type="button"
        disabled={trimmed === '' || !changed}
        onClick={() => {
          onRename(trimmed, description.trim());
        }}
      >
        {t('template.rename')}
      </button>
      <button type="button" onClick={onDelete}>
        {t('template.delete')}
      </button>
    </li>
  );
};

export const TemplateManager = ({
  siteId,
  siteSettings,
  onClose,
}: TemplateManagerProps): ReactElement => {
  const { t } = useTranslator(sites);
  const [templates, setTemplates] = useState(() => savedTemplates(siteSettings));
  const { state: saveState, save } = useSave();

  const persist = (next: readonly SavedTemplate[]): void => {
    save(async () => {
      await api.patchSite(
        siteId,
        encodeSiteSettings({ [SAVED_TEMPLATES_KEY]: savedTemplatesWire(next) }),
      );
      setTemplates(next);
    });
  };

  return (
    <div className="template-dialog-backdrop">
      <div
        className="template-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t('template.manager')}
      >
        <h2>{t('template.manager')}</h2>
        <p>{t('template.managerHint')}</p>
        <SaveStatus state={saveState} />
        {templates.length === 0 ? (
          <p>{t('template.empty')}</p>
        ) : (
          <ul className="template-manager-list">
            {templates.map((template) => (
              <TemplateRow
                key={template.id}
                template={template}
                onRename={(name, description) => {
                  persist(
                    templates.map((entry) =>
                      entry.id === template.id ? { ...entry, name, description } : entry,
                    ),
                  );
                }}
                onDelete={() => {
                  persist(templates.filter((entry) => entry.id !== template.id));
                }}
              />
            ))}
          </ul>
        )}
        <div className="template-dialog-actions">
          <button type="button" onClick={onClose}>
            {t('common.close')}
          </button>
        </div>
      </div>
    </div>
  );
};
