/**
 * The insert picker: a searchable list of the registered layouts and the page
 * templates. Choosing a layout closes the picker and hands the layout name and
 * the insertion index back to the canvas, which dispatches `addBlock`; choosing
 * a template hands the template and the index back, and the canvas dispatches
 * `insertBlocks`.
 */
import { useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import type { BlockRegistry } from '../../blockSchema';
import type { TemplateChoice } from '../../templates';

export interface BlockPickerProps {
  readonly registry: BlockRegistry;
  readonly templates: readonly TemplateChoice[];
  /** The flat index the choice is inserted at. */
  readonly index: number;
  readonly onChoose: (layout: string, index: number) => void;
  readonly onChooseTemplate: (template: TemplateChoice, index: number) => void;
  readonly onClose: () => void;
}

const matches = (label: string, name: string, query: string): boolean => {
  const needle = query.trim().toLowerCase();
  return (
    needle === '' ||
    label.toLowerCase().includes(needle) ||
    name.toLowerCase().includes(needle)
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
  const { t } = useTranslator();
  const [query, setQuery] = useState('');
  const options = [...registry.layouts, ...registry.romeka].filter((layout) =>
    matches(t(layout.label), layout.name, query),
  );
  const templateOptions = templates.filter((template) =>
    matches(template.name, template.id, query),
  );
  const searchId = `canvas-picker-search-${String(index)}`;

  const onKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  };

  return (
    <div className="canvas-picker" role="dialog" aria-label={t('canvas.picker')}>
      <p>
        <label htmlFor={searchId}>{t('canvas.search')}</label>
        <input
          id={searchId}
          type="search"
          value={query}
          autoFocus
          onChange={(event) => {
            setQuery(event.target.value);
          }}
          onKeyDown={onKeyDown}
        />
      </p>
      {options.length === 0 && templateOptions.length === 0 ? (
        <p className="canvas-picker-empty">{t('canvas.noMatches')}</p>
      ) : null}
      {options.length === 0 ? null : (
        <>
          <h4 className="canvas-picker-heading">{t('canvas.pickerBlocks')}</h4>
          <ul className="canvas-picker-list">
            {options.map((layout) => (
              <li key={layout.name}>
                <button
                  type="button"
                  onClick={() => {
                    onChoose(layout.name, index);
                  }}
                >
                  {t(layout.label)}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      {templateOptions.length === 0 ? null : (
        <>
          <h4 className="canvas-picker-heading">{t('template.insert')}</h4>
          <ul className="canvas-picker-list">
            {templateOptions.map((template) => (
              <li key={template.id}>
                <button
                  type="button"
                  onClick={() => {
                    onChooseTemplate(template, index);
                  }}
                >
                  {template.name}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      <p>
        <button
          type="button"
          onClick={() => {
            onClose();
          }}
        >
          {t('canvas.close')}
        </button>
      </p>
    </div>
  );
};
