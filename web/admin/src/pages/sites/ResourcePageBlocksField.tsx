/**
 * The resource page section editor: for items, item sets and media, the
 * ordered sections that page shows, and below them the sections it hides. It
 * edits the theme's own settings map (`resource_page_blocks`), which
 * `crates/omeka-site/src/resource_page.rs` reads when it renders a resource
 * page.
 *
 * Each row says what a reader sees. A shown section moves by its handle or its
 * arrows (`useReorderable`) and hides with one button; a hidden section comes
 * back with "Show" at the end of the list. Hiding is the stored map leaving the
 * name out, so nothing new is stored.
 *
 * The control is a controlled view: it reads a draft and every edit leaves as
 * the whole draft through `onChange`, the way the block-list fields do.
 */
import type { ReactElement } from 'react';
import {
  ReorderButtons,
  ReorderHandle,
  ReorderStatus,
  useReorderable,
} from '../../app/reorder/useReorderable';
import { useTranslator } from '../../i18n';
import type {
  ResourcePageBlockSpec,
  ResourcePageBlocksDraft,
  ResourcePageBlocksRegistry,
  ResourcePageResourceSpec,
} from './resourcePageBlocksSchema';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

export interface ResourcePageBlocksFieldProps {
  readonly registry: ResourcePageBlocksRegistry;
  readonly drafts: ResourcePageBlocksDraft;
  readonly onChange: (drafts: ResourcePageBlocksDraft) => void;
}

/** A section's name and its one line, as both lists show them. */
const SectionText = ({ title, help }: { readonly title: string; readonly help: string }): ReactElement => (
  <span className="resource-page-block-text">
    <span className="resource-page-block-label">{title}</span>
    <span className="resource-page-block-help">{help}</span>
  </span>
);

/** One resource's shown sections, then its hidden ones. */
const ResourceBlocks = ({
  resource,
  names,
  onChange,
}: {
  readonly resource: ResourcePageResourceSpec;
  readonly names: readonly string[];
  readonly onChange: (names: readonly string[]) => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const resourceLabel = t(resource.labelKey);
  const specOf = (name: string): ResourcePageBlockSpec | undefined =>
    resource.blocks.find((block) => block.name === name);
  const labelOf = (name: string): string => {
    const spec = specOf(name);
    return spec === undefined ? t('resourcePage.unknown', { name }) : t(spec.labelKey);
  };
  const reorder = useReorderable({ items: names, onReorder: onChange, labelOf });
  const hidden = resource.blocks.filter((block) => !names.includes(block.name));

  return (
    <fieldset className="resource-page-resource">
      <legend>{resourceLabel}</legend>
      {names.length === 0 ? (
        <p className="field-note">{t('resourcePage.empty')}</p>
      ) : (
        <ol
          className="resource-page-blocks"
          aria-label={t('resourcePage.blockList', { resource: resourceLabel })}
        >
          {names.map((name, index) => {
            const spec = specOf(name);
            const label = labelOf(name);
            return (
              <li className="resource-page-block" key={name} {...reorder.row(index)}>
                <ReorderHandle reorder={reorder} index={index} />
                <SectionText
                  title={label}
                  help={spec === undefined ? t('resourcePage.unknownHelp') : t(spec.helpKey)}
                />
                <ReorderButtons reorder={reorder} index={index} />
                <button
                  type="button"
                  className="link-button"
                  aria-label={t('resourcePage.hideLabel', { label })}
                  onClick={() => {
                    onChange(names.filter((_, at) => at !== index));
                  }}
                >
                  {t('resourcePage.hide')}
                </button>
              </li>
            );
          })}
        </ol>
      )}
      <ReorderStatus reorder={reorder} />
      {hidden.length === 0 ? null : (
        <div className="resource-page-hidden">
          <h4>{t('resourcePage.hidden')}</h4>
          <ul
            className="resource-page-blocks"
            aria-label={t('resourcePage.hiddenList', { resource: resourceLabel })}
          >
            {hidden.map((block) => {
              const label = t(block.labelKey);
              return (
                <li className="resource-page-block" key={block.name}>
                  <SectionText title={label} help={t(block.helpKey)} />
                  <button
                    type="button"
                    className="link-button"
                    aria-label={t('resourcePage.showLabel', { label })}
                    onClick={() => {
                      onChange([...names, block.name]);
                    }}
                  >
                    {t('resourcePage.show')}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </fieldset>
  );
};

export const ResourcePageBlocksField = ({
  registry,
  drafts,
  onChange,
}: ResourcePageBlocksFieldProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  return (
    <fieldset className="resource-page-blocks-field">
      <legend>{t('resourcePage.title')}</legend>
      <p className="field-note">{t('resourcePage.hint')}</p>
      {registry.resources.map((resource) => (
        <ResourceBlocks
          key={resource.key}
          resource={resource}
          names={drafts[resource.key] ?? []}
          onChange={(names) => {
            onChange({ ...drafts, [resource.key]: names });
          }}
        />
      ))}
    </fieldset>
  );
};
