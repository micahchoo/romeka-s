/**
 * The value map of one field's rule, of one free set or of one set level
 * (`SlotRule.map`, `EachSet.map`): each value the sources give, compared as a title, becomes
 * another text, or nothing (`""`). One control shows it in every place
 * (`value-map.tsx`); these are its edits. A join is an entry whose text is
 * another value of the list, so two spellings, two item types or two sets
 * become one by the same act. The wire shape of the map is v3's.
 */
import type { ImportPlan, LevelId, RecordKind, SetSource, SlotName } from '../../api/import-plan';
import { written } from './advanced/writes';
import { seriesOf, setLevelsOf, setsOf, withSeries, withSetLevel, withSets } from './series';
import { fold } from './simple/list-plan';

export type ValueMap = Readonly<Record<string, string>>;

/**
 * Where a map lives: one field's rule of a series, the free set at
 * `freeSet`, or the set level `setLevel` around the one series `series`.
 * The last two make one set per value.
 */
export type MapHome =
  | { readonly level: LevelId; readonly kind: RecordKind; readonly slot: SlotName }
  | { readonly freeSet: number }
  | { readonly setLevel: LevelId; readonly series: LevelId };

/** How the run reads a mapped text: as the text itself, an item type's label, a set's title, or a class's term. */
export type Becomes = 'text' | 'type' | 'set' | 'class';

export const slotOf = (home: MapHome): SlotName => ('slot' in home ? home.slot : 'sets');

export const becomesOf = (slot: SlotName): Becomes => {
  switch (slot) {
    case 'template':
      return 'type';
    case 'sets':
      return 'set';
    case 'class':
      return 'class';
    default:
      return 'text';
  }
};

type EachSet = Extract<SetSource, { each: unknown }>;

const asEach = (set: SetSource | null | undefined): EachSet | null => (set !== null && set !== undefined && 'each' in set ? set : null);

/** The one set per value a set home makes. */
export const eachOf = (plan: ImportPlan, home: Exclude<MapHome, { readonly slot: SlotName }>): EachSet | null =>
  asEach('freeSet' in home ? setsOf(plan)[home.freeSet] : setLevelsOf(plan).find((level) => level.id === home.setLevel)?.set);

export const mapOf = (plan: ImportPlan, home: MapHome): ValueMap =>
  'slot' in home ? (seriesOf(plan, home.level).slots[home.kind][home.slot]?.map ?? {}) : (eachOf(plan, home)?.each.map ?? {});

/** What the map makes of `value`: `undefined` when no key folds to it. */
export const mappedOf = (map: ValueMap, value: string): string | undefined => {
  const key = Object.keys(map).find((one) => fold(one) === fold(value));
  return key === undefined ? undefined : map[key];
};

/**
 * `value` mapped to `to`, or unmapped for `undefined`. Two keys that fold
 * alike are refused on save, so every old key of the value goes first.
 */
export const withMapped = (map: ValueMap, value: string, to: string | undefined): ValueMap => {
  const rest = Object.entries(map).filter(([key]) => fold(key) !== fold(value));
  return Object.fromEntries(to === undefined ? rest : [...rest, [value, to]]);
};

/** The plan with `map` at `home`; an empty map is absent. A rule or a free set the plan does not hold stays absent. */
export const withMap = (plan: ImportPlan, home: MapHome, map: ValueMap): ImportPlan => {
  const empty = Object.keys(map).length === 0;
  const mapped = ({ each: { sources, combine, names } }: EachSet): EachSet => ({ each: empty ? { sources, combine, names } : { sources, combine, names, map } });
  if ('freeSet' in home) {
    return eachOf(plan, home) === null
      ? plan
      : written(plan, 'valueMap', undefined, (now) =>
          withSets(now, (sets) =>
            sets.map((set, at) => {
              const each = asEach(set);
              return at === home.freeSet && each !== null ? mapped(each) : set;
            }),
          ),
        );
  }
  if ('setLevel' in home) {
    const each = eachOf(plan, home);
    return each === null ? plan : written(plan, 'valueMap', undefined, (now) => withSetLevel(now, home.setLevel, mapped(each)));
  }
  const { level, kind, slot } = home;
  const rule = seriesOf(plan, level).slots[kind][slot];
  if (rule === undefined) {
    return plan;
  }
  const { sources, combine } = rule;
  return written(plan, 'valueMap', level, (now) =>
    withSeries(now, level, (series) => ({ ...series, slots: { ...series.slots, [kind]: { ...series.slots[kind], [slot]: empty ? { sources, combine } : { sources, combine, map } } } })),
  );
};

/** `value` at `home` made `to`, or as written for `undefined`. */
export const withEntry = (plan: ImportPlan, home: MapHome, value: string, to: string | undefined): ImportPlan =>
  withMap(plan, home, withMapped(mapOf(plan, home), value, to));

/** A new item type the map names, made by the run (`from: "mapping"`); one the plan already makes is not added twice. */
export const withNewType = (plan: ImportPlan, label: string): ImportPlan =>
  plan.newTypes.some((type) => fold(type.label) === fold(label)) ? plan : { ...plan, newTypes: [...plan.newTypes, { label, from: 'mapping', class: null }] };
