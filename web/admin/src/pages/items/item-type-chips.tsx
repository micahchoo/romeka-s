/**
 * The first filter of the Items browse: one chip per item type, each with how many items it has,
 * and "No item type" while some item has none. A press filters to that type; a press on the
 * chosen chip lets it go. The names come from the option lists, so the chips filter before the
 * counts arrive, and a failed count leaves the chips working without numbers.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useId, useRef } from 'react';
import type { ItemTypeCounts } from '../../api/places';
import type { TemplateId } from '../../api/types';
import { useTranslator } from '../../i18n';
import { resources } from '../../i18n/namespaces/resources';
import { NONE } from './browse';
import type { None } from './browse';
import type { BrowseOptions } from './browse-options';
import type { OnFilter } from './filter-value';

/** Where the chip counts stand. A reload keeps the last counts on screen. */
export type ChipCounts =
  | { readonly status: 'loading' }
  | { readonly status: 'failed' }
  | { readonly status: 'ready'; readonly counts: ItemTypeCounts };

interface Chip {
  readonly value: TemplateId | None;
  readonly name: string;
  readonly count: number | null;
}

const chipsOf = (
  options: BrowseOptions,
  counts: ChipCounts,
  chosen: TemplateId | None | null,
  noTypeName: string,
): readonly Chip[] => {
  const ready = counts.status === 'ready' ? counts.counts : null;
  const types = [...options.templates]
    .map(([id, name]): Chip => ({ value: id, name, count: ready === null ? null : (ready.items.get(id) ?? 0) }))
    .sort((a, b) => a.name.localeCompare(b.name));
  const untyped = ready?.noItemType ?? 0;
  return untyped > 0 || chosen === NONE
    ? [...types, { value: NONE, name: noTypeName, count: ready === null ? null : untyped }]
    : types;
};

/** Arrow keys, Home and End move focus between the chips; choosing stays a press. */
const STEPS: Readonly<Record<string, (at: number, last: number) => number>> = {
  ArrowRight: (at, last) => (at === last ? 0 : at + 1),
  ArrowDown: (at, last) => (at === last ? 0 : at + 1),
  ArrowLeft: (at, last) => (at === 0 ? last : at - 1),
  ArrowUp: (at, last) => (at === 0 ? last : at - 1),
  Home: () => 0,
  End: (_, last) => last,
};

export const ItemTypeChips = ({
  chosen,
  options,
  optionsReady,
  counts,
  onFilter,
}: {
  readonly chosen: TemplateId | None | null;
  readonly options: BrowseOptions;
  /** The option lists arrived, so an empty type list means the installation has none. */
  readonly optionsReady: boolean;
  readonly counts: ChipCounts;
  readonly onFilter: OnFilter;
}): ReactElement | null => {
  const { t, number } = useTranslator(resources);
  const labelId = useId();
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  if (!optionsReady) {
    return null;
  }
  if (options.templates.size === 0) {
    return (
      <p className="item-type-chips-empty">
        {t('browse.noTypes')} <a href="#/resource-templates">{t('browse.noTypesLink')}</a>
      </p>
    );
  }
  const chips = chipsOf(options, counts, chosen, t('browse.templateNone'));
  const focusable = Math.max(
    0,
    chips.findIndex((chip) => chip.value === chosen),
  );
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, at: number): void => {
    const step = STEPS[event.key];
    if (step === undefined) {
      return;
    }
    event.preventDefault();
    buttons.current[step(at, chips.length - 1)]?.focus();
  };
  return (
    <div className="item-type-chips">
      <span id={labelId} className="item-type-chips-label">
        {t('browse.template')}
      </span>
      <div role="radiogroup" aria-labelledby={labelId} className="item-type-chip-row">
        {chips.map((chip, at) => {
          const checked = chip.value === chosen;
          const total =
            chip.count === null
              ? undefined
              : chip.value === NONE
                ? t(chip.count === 1 ? 'browse.noTypeTotalOne' : 'browse.noTypeTotalMany', { count: number(chip.count) })
                : t(chip.count === 1 ? 'browse.typeTotalOne' : 'browse.typeTotalMany', { count: number(chip.count) });
          return (
            <button
              key={String(chip.value)}
              ref={(node) => {
                buttons.current[at] = node;
              }}
              type="button"
              role="radio"
              aria-checked={checked}
              tabIndex={at === focusable ? 0 : -1}
              className="item-type-chip"
              title={total}
              onClick={() => {
                onFilter({ templateId: checked ? null : chip.value });
              }}
              onKeyDown={(event) => {
                onKeyDown(event, at);
              }}
            >
              {checked ? <span aria-hidden="true">✓</span> : null}
              <span className="item-type-chip-name">{chip.name}</span>
              {counts.status === 'failed' ? null : (
                <>
                  {' '}
                  <span className="item-type-chip-count">
                    {chip.count === null ? '…' : number(chip.count)}
                  </span>
                </>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
};
