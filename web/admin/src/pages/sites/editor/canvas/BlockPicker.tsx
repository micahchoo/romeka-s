/**
 * The insert picker: a searchable dialog of the registered layouts and the page
 * templates, each with the one line that says what it adds. Choosing a layout
 * closes the picker and hands the layout name and the insertion index back to
 * the canvas, which dispatches `addBlock`; choosing a template hands the
 * template and the index back, and the canvas dispatches `insertBlocks`.
 *
 * The dialog sits over the page at a fixed size and scrolls its own list, so
 * every option is reachable wherever the insertion point is. A template with no
 * blocks inserts nothing, so it is offered only by the new-page chooser.
 */
import { useEffect, useId, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from 'react';
import type { JsonValue } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import type { BlockRegistry } from '../../blockSchema';
import { builtinChoices, savedChoices, savedTemplates } from '../../templates';
import type { TemplateChoice } from '../../templates';
import { BUILTIN_TEMPLATES } from '../../templateRegistry';
import { editor } from '../../../../i18n/namespaces/editor';
import { sites } from '../../../../i18n/namespaces/sites';
import './picker.css';

/** The templates an insert point offers: the built-in ones, then the site's saved ones.
 * The Outline and the preview both insert through it, so they offer the same list. */
export const useInsertChoices = (siteSettings: JsonValue | null): readonly TemplateChoice[] => {
  const { t } = useTranslator(sites, editor);
  return [...builtinChoices(BUILTIN_TEMPLATES, t), ...savedChoices(savedTemplates(siteSettings))];
};

export interface BlockPickerProps {
  readonly registry: BlockRegistry;
  readonly templates: readonly TemplateChoice[];
  /** The flat index the choice is inserted at. */
  readonly index: number;
  readonly onChoose: (layout: string, index: number) => void;
  readonly onChooseTemplate: (template: TemplateChoice, index: number) => void;
  readonly onClose: () => void;
}

const matches = (query: string, ...texts: readonly string[]): boolean => {
  const needle = query.trim().toLowerCase();
  return needle === '' || texts.some((text) => text.toLowerCase().includes(needle));
};

/** One option: the name is the button's accessible name, the line its description. */
const PickerOption = ({
  name,
  description,
  onClick,
}: {
  readonly name: string;
  readonly description: string;
  readonly onClick: () => void;
}): ReactElement => {
  const id = useId();
  return (
    <li>
      <button
        type="button"
        className="canvas-picker-option"
        aria-labelledby={`${id}-name`}
        aria-describedby={`${id}-description`}
        onClick={onClick}
      >
        <span id={`${id}-name`} className="canvas-picker-name">
          {name}
        </span>
        <span id={`${id}-description`} className="canvas-picker-description">
          {description}
        </span>
      </button>
    </li>
  );
};

export const BlockPicker = ({
  registry,
  templates,
  index,
  onChoose,
  onChooseTemplate,
  onClose,
}: BlockPickerProps): ReactElement => {
  const { t } = useTranslator(sites, editor);
  const [query, setQuery] = useState('');
  const options = [...registry.layouts, ...registry.romeka].filter((layout) =>
    matches(query, t(layout.label), layout.name, t(layout.description)),
  );
  const templateOptions = templates.filter(
    (template) =>
      template.blocks.length > 0 &&
      matches(query, template.name, template.id, template.description),
  );
  const searchId = `canvas-picker-search-${String(index)}`;

  // The insert button that opened the picker gets focus back when it closes.
  useEffect(() => {
    const opener = document.activeElement;
    return () => {
      if (opener instanceof HTMLElement) {
        opener.focus();
      }
    };
  }, []);

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  };

  return (
    <div
      className="canvas-picker-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        className="canvas-picker"
        role="dialog"
        aria-modal="true"
        aria-label={t('canvas.picker')}
        onKeyDown={onKeyDown}
      >
        <div className="canvas-picker-head">
          <label htmlFor={searchId}>{t('canvas.search')}</label>
          <input
            id={searchId}
            type="search"
            value={query}
            autoFocus
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
        </div>
        <div className="canvas-picker-body">
          {options.length === 0 && templateOptions.length === 0 ? (
            <p className="canvas-picker-empty">{t('canvas.noMatches')}</p>
          ) : null}
          {options.length === 0 ? null : (
            <>
              <h4 className="canvas-picker-heading">{t('canvas.pickerBlocks')}</h4>
              <ul className="canvas-picker-list">
                {options.map((layout) => (
                  <PickerOption
                    key={layout.name}
                    name={t(layout.label)}
                    description={t(layout.description)}
                    onClick={() => {
                      onChoose(layout.name, index);
                    }}
                  />
                ))}
              </ul>
            </>
          )}
          {templateOptions.length === 0 ? null : (
            <>
              <h4 className="canvas-picker-heading">{t('template.insert')}</h4>
              <ul className="canvas-picker-list">
                {templateOptions.map((template) => (
                  <PickerOption
                    key={template.id}
                    name={template.name}
                    description={template.description}
                    onClick={() => {
                      onChooseTemplate(template, index);
                    }}
                  />
                ))}
              </ul>
            </>
          )}
        </div>
        <div className="canvas-picker-foot">
          <button type="button" onClick={onClose}>
            {t('canvas.close')}
          </button>
        </div>
      </div>
    </div>
  );
};
