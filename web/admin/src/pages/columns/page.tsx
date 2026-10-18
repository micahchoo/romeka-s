import type { ReactElement } from 'react';
import { useState } from 'react';
import { TableScroll } from '../../app/table-scroll';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';

type BrowseResource = 'items' | 'item_sets' | 'media' | 'users';

/** The browse columns each resource type renders (`browse()` header rows). */
export const BROWSE_COLUMNS: Readonly<Record<BrowseResource, readonly MessageKey[]>> = {
  items: ['column.title', 'column.resourceClass', 'column.owner', 'column.created'],
  item_sets: ['column.title', 'column.owner', 'column.created', 'column.public', 'column.open'],
  media: [
    'column.preview',
    'column.title',
    'column.item',
    'column.mediaType',
    'column.source',
    'column.ingester',
  ],
  users: ['column.name', 'column.email', 'column.role', 'column.active', 'column.created'],
};

const RESOURCES: readonly { readonly value: BrowseResource; readonly labelKey: MessageKey }[] = [
  { value: 'items', labelKey: 'page.items' },
  { value: 'item_sets', labelKey: 'page.itemSets' },
  { value: 'media', labelKey: 'page.media' },
  { value: 'users', labelKey: 'page.users' },
];

const isBrowseResource = (value: string): value is BrowseResource =>
  value === 'items' || value === 'item_sets' || value === 'media' || value === 'users';

/**
 * `Admin/ColumnsController::columnListAction`: which browse columns show for a
 * resource type. The port serves no column-settings endpoint, so the choice
 * lives in this page only.
 */
export const ColumnsPage = (): ReactElement => {
  const { t } = useTranslator();
  const [resource, setResource] = useState<BrowseResource>('items');
  const [hidden, setHidden] = useState<readonly MessageKey[]>([]);

  const columns = BROWSE_COLUMNS[resource];
  const visible = columns.filter((column) => !hidden.includes(column));

  const toggle = (column: MessageKey): void => {
    setHidden((previous) =>
      previous.includes(column)
        ? previous.filter((entry) => entry !== column)
        : [...previous, column],
    );
  };

  return (
    <section>
      <p>
        <label htmlFor="columns-resource">{t('columns.resourceType')}</label>
        <select
          id="columns-resource"
          value={resource}
          onChange={(event) => {
            const value = event.target.value;
            if (isBrowseResource(value)) {
              setResource(value);
              setHidden([]);
            }
          }}
        >
          {RESOURCES.map((option) => (
            <option key={option.value} value={option.value}>
              {t(option.labelKey)}
            </option>
          ))}
        </select>
      </p>
      <fieldset>
        <legend>{t('columns.legend')}</legend>
        {columns.map((column) => (
          <label key={column}>
            <input
              type="checkbox"
              checked={!hidden.includes(column)}
              onChange={() => {
                toggle(column);
              }}
            />
            {t(column)}
          </label>
        ))}
      </fieldset>
      <h2>{t('action.preview')}</h2>
      <TableScroll label={t('action.preview')}>
        <table>
          <thead>
            <tr>
              {visible.map((column) => (
                <th key={column}>{t(column)}</th>
              ))}
            </tr>
          </thead>
        </table>
      </TableScroll>
    </section>
  );
};
