/**
 * A value shown in a row or a card that is also its own filter: a class, an
 * owner, an item set, the Private cue. Clicking it narrows the list to items
 * that share it, so the reader filters from where the value is.
 */
import type { ReactElement } from 'react';
import type { BrowseFilters } from './browse';

export type OnFilter = (change: Partial<BrowseFilters>) => void;

export interface FilterValueProps {
  readonly label: string;
  /** What the click does, in words: "Show only items with this class". */
  readonly title: string;
  readonly change: Partial<BrowseFilters>;
  readonly onFilter: OnFilter;
  readonly className?: string;
}

export const FilterValue = ({ label, title, change, onFilter, className }: FilterValueProps): ReactElement => (
  <button
    type="button"
    className={className === undefined ? 'filter-value' : `filter-value ${className}`}
    title={title}
    onClick={() => {
      onFilter(change);
    }}
  >
    {label}
  </button>
);
