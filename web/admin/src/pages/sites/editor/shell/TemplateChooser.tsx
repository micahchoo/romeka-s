/**
 * The create screen's template chooser: the built-in starting points and the
 * site's own saved templates, each card showing the blocks it lays down.
 * Choosing one hands the template to the shell, which applies it to the empty
 * draft; choosing blank opens the editor with no blocks.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import type { JsonValue } from '../../../../api/types';
import type { BlockRegistry } from '../../blockSchema';
import { builtinChoices, savedChoices, savedTemplates } from '../../templates';
import type { TemplateChoice } from '../../templates';
import { BUILTIN_TEMPLATES } from '../../templateRegistry';
import { templateOutline } from '../canvas/summary';

export interface TemplateChooserProps {
  readonly registry: BlockRegistry;
  /** The site's `o:site_settings`, which carry its saved templates. */
  readonly siteSettings: JsonValue | null;
  readonly onChoose: (template: TemplateChoice) => void;
  readonly onBlank: () => void;
}

const TemplateCard = ({
  registry,
  template,
  onChoose,
}: {
  readonly registry: BlockRegistry;
  readonly template: TemplateChoice;
  readonly onChoose: (template: TemplateChoice) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const outline = templateOutline(registry, template.blocks, t);
  return (
    <li className="template-card">
      <button
        type="button"
        onClick={() => {
          onChoose(template);
        }}
      >
        <span className="template-card-name">{template.name}</span>
        <span className="template-card-description">{template.description}</span>
        <span className="template-card-outline">
          {outline.length === 0 ? t('template.noBlocks') : outline.join(' \u203a ')}
        </span>
      </button>
    </li>
  );
};

const TemplateList = ({
  registry,
  templates,
  onChoose,
}: {
  readonly registry: BlockRegistry;
  readonly templates: readonly TemplateChoice[];
  readonly onChoose: (template: TemplateChoice) => void;
}): ReactElement => (
  <ul className="template-list">
    {templates.map((template) => (
      <TemplateCard
        key={template.id}
        registry={registry}
        template={template}
        onChoose={onChoose}
      />
    ))}
  </ul>
);

export const TemplateChooser = ({
  registry,
  siteSettings,
  onChoose,
  onBlank,
}: TemplateChooserProps): ReactElement => {
  const { t } = useTranslator();
  const saved = savedTemplates(siteSettings);
  return (
    <section className="template-chooser" aria-label={t('template.choose')}>
      <h2>{t('template.choose')}</h2>
      <p>{t('template.chooseHint')}</p>
      <h3>{t('template.builtin')}</h3>
      <TemplateList
        registry={registry}
        templates={builtinChoices(BUILTIN_TEMPLATES, t)}
        onChoose={onChoose}
      />
      {saved.length === 0 ? null : (
        <>
          <h3>{t('template.saved')}</h3>
          <TemplateList registry={registry} templates={savedChoices(saved)} onChoose={onChoose} />
        </>
      )}
      <p>
        <button type="button" onClick={onBlank}>
          {t('template.blankChoice')}
        </button>
      </p>
    </section>
  );
};
