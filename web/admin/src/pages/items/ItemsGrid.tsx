/**
 * The Items browse grid: one card per item, its picture first, for the reader
 * who knows an item by how it looks rather than by its title.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import type { Item } from '../../api/types';
import { classLabelOf } from '../../app/labels';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import { FilterValue } from './filter-value';
import { PrivateCue } from './PrivateCue';
import type { ItemsViewProps } from './ItemsTable';

/**
 * The item's picture, or a neutral tile that says why there is none: the item
 * has no media, or it has media and no picture could be shown.
 */
const CardThumb = ({ item }: { readonly item: Item }): ReactElement => {
  const { t } = useTranslator(resources);
  const [failed, setFailed] = useState(false);
  const url = item.thumbnailDisplayUrls.medium ?? null;
  if (url !== null && !failed) {
    return (
      <img
        className="item-card-thumb"
        src={url}
        alt=""
        loading="lazy"
        onError={() => {
          setFailed(true);
        }}
      />
    );
  }
  return (
    <span className="item-card-thumb item-card-empty">
      {item.media.length === 0 ? t('browse.mediaNone') : t('browse.noPreview')}
    </span>
  );
};

export const ItemsGrid = ({
  items,
  options,
  picked,
  onPick,
  onPickPage,
  onFilter,
  hrefOf,
}: ItemsViewProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const text = useTermLabels();
  const allPicked = items.length > 0 && items.every((item) => picked.includes(item.id));
  return (
    <>
      <p className="item-grid-pick-all">
        <label>
          <input
            type="checkbox"
            checked={allPicked}
            onChange={(event) => {
              onPickPage(event.target.checked);
            }}
          />{' '}
          {t('items.selectAll')}
        </label>
      </p>
      <ul className="item-grid" aria-label={t('resource.items')}>
        {items.map((item) => {
          const title = item.title ?? t('common.untitled');
          const className = classLabelOf(options.classNames, item.resourceClass, text);
          return (
            <li key={item.id} className="item-card">
              <CardThumb item={item} />
              <input
                type="checkbox"
                className="item-card-pick"
                aria-label={t('items.selectItem', { title })}
                checked={picked.includes(item.id)}
                onChange={(event) => {
                  onPick(item.id, event.target.checked);
                }}
              />
              <a className="item-card-title" href={hrefOf(item)}>
                {title}
              </a>
              <span className="item-card-facts">
                {className === null || item.resourceClass === null ? null : (
                  <FilterValue
                    label={className}
                    title={t('browse.onlyClass')}
                    change={{ classId: item.resourceClass.id }}
                    onFilter={onFilter}
                  />
                )}
                {item.isPublic ? null : <PrivateCue onFilter={onFilter} />}
              </span>
            </li>
          );
        })}
      </ul>
    </>
  );
};
