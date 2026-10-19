/**
 * The resource page block editor: for items, item sets and media, the ordered
 * list of block layouts that resource page renders, with add, remove and
 * reorder. It edits the theme's own settings map (`resource_page_blocks`), which
 * `crates/omeka-site/src/resource_page.rs` reads when it renders a resource
 * page.
 *
 * The control is a controlled view: it reads a draft and every edit leaves as
 * the whole draft through `onChange`, the way the block-list fields do.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { useTranslator } from '../../i18n';
import type {
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

const move = (names: readonly string[], index: number, delta: number): readonly string[] => {
  const target = index + delta;
  const moved = names[index];
  if (moved === undefined || target < 0 || target >= names.length) {
    return names;
  }
  const next = [...names];
  next.splice(index, 1);
  next.splice(target, 0, moved);
  return next;
};

/** One resource's ordered list, its add control and its reorder buttons. */
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
  const [pending, setPending] = useState('');
  const resourceLabel = t(resource.labelKey);
  const labelOf = (name: string): string => {
    const spec = resource.blocks.find((block) => block.name === name);
    return spec === undefined ? t('resourcePage.unknown', { name }) : t(spec.labelKey);
  };
  const addable = resource.blocks.filter((block) => !names.includes(block.name));

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
            const label = labelOf(name);
            return (
              <li className="resource-page-block" key={`${name}-${String(index)}`}>
                <div className="resource-page-block-row">
                  <span className="resource-page-block-label">{label}</span>
                  <button
                    type="button"
                    disabled={index === 0}
                    aria-label={t('resourcePage.moveUp', { label })}
                    onClick={() => {
                      onChange(move(names, index, -1));
                    }}
                  >
                    {t('navigation.moveUpShort')}
                  </button>
                  <button
                    type="button"
                    disabled={index === names.length - 1}
                    aria-label={t('resourcePage.moveDown', { label })}
                    onClick={() => {
                      onChange(move(names, index, 1));
                    }}
                  >
                    {t('navigation.moveDownShort')}
                  </button>
                  <button
                    type="button"
                    aria-label={t('resourcePage.remove', { label })}
                    onClick={() => {
                      onChange(names.filter((_, at) => at !== index));
                    }}
                  >
                    {t('action.remove')}
                  </button>
                </div>
              </li>
            );
          })}
        </ol>
      )}
      <p className="resource-page-add">
        <select
          aria-label={t('resourcePage.addLabel', { resource: resourceLabel })}
          value={pending}
          onChange={(event) => {
            setPending(event.target.value);
          }}
        >
          <option value="">{t('resourcePage.addPlaceholder')}</option>
          {addable.map((block) => (
            <option key={block.name} value={block.name}>
              {t(block.labelKey)}
            </option>
          ))}
        </select>
        <button
          type="button"
          disabled={pending === ''}
          onClick={() => {
            onChange([...names, pending]);
            setPending('');
          }}
        >
          {t('resourcePage.add')}
        </button>
      </p>
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
