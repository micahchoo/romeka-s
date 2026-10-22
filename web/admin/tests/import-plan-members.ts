/**
 * The members of a v3 plan's one series, by the names the test servers and
 * their scenarios read and write: `grouping`, `example`, `sheets` (the
 * series' lists) and `slots`, with the item sets rule (the first `each` free
 * set) as `slots.item.sets`, and `simple` (the series' Simple state). The
 * servers stand in for the server's own reads of a plan, so they read and
 * write it as raw JSON, not through the seam.
 */
import { asRecord, isRecord } from '../src/api/parse';

type Json = Record<string, unknown>;

const levelsOf = (plan: Json): Json[] => {
  const structure = isRecord(plan['structure']) ? plan['structure'] : {};
  return Array.isArray(structure['levels']) ? structure['levels'].filter(isRecord) : [];
};

const freeSetsOf = (plan: Json): Json[] => {
  const structure = isRecord(plan['structure']) ? plan['structure'] : {};
  return Array.isArray(structure['freeSets']) ? structure['freeSets'].filter(isRecord) : [];
};

const seriesOf = (plan: Json): Json => {
  const [series, ...more] = levelsOf(plan);
  if (series === undefined || more.length > 0 || series['kind'] !== 'series') {
    throw new Error('the test servers read a plan of one series');
  }
  return series;
};

/** The plan with its one series' members flat beside the others, as the scenarios name them. */
export const flatPlan = (plan: Json): Json => {
  const series = seriesOf(plan);
  const slots = asRecord(series['slots'], []);
  const rule = freeSetsOf(plan)
    .map((set) => set['each'])
    .find(isRecord);
  const sets = rule === undefined ? {} : { sets: Object.fromEntries(Object.entries(rule).filter(([key]) => key !== 'names')) };
  const id = String(series['id']);
  const simple = isRecord(plan['simple']) ? plan['simple'][id] : undefined;
  const rest = Object.fromEntries(Object.entries(plan).filter(([key]) => key !== 'simple'));
  return {
    ...rest,
    ...(simple === undefined ? {} : { simple }),
    grouping: series['grouping'],
    example: series['example'],
    sheets: series['lists'],
    slots: { ...slots, item: { ...asRecord(slots['item'], []), ...sets } },
  };
};

/** The plan with these members of its one series replaced; an item sets rule goes to the free sets, naming sets by title. */
export const withMembers = (plan: Json, members: Json): Json => {
  const series = seriesOf(plan);
  const { grouping, example, sheets, slots, simple, ...rest } = members;
  const item = isRecord(slots) && isRecord(slots['item']) ? slots['item'] : undefined;
  const { sets, ...itemRules } = item ?? {};
  const edited: Json = {
    ...series,
    ...(grouping === undefined ? {} : { grouping }),
    ...(example === undefined ? {} : { example }),
    ...(sheets === undefined ? {} : { lists: sheets }),
    ...(isRecord(slots) ? { slots: { ...slots, item: itemRules } } : {}),
  };
  const structure = asRecord(plan['structure'], []);
  const freeSets = isRecord(sets) ? [{ each: { ...sets, names: 'title' } }] : structure['freeSets'];
  const states = simple === undefined ? {} : { simple: { ...(isRecord(plan['simple']) ? plan['simple'] : {}), [String(series['id'])]: simple } };
  return { ...plan, ...rest, ...states, structure: { ...structure, levels: [edited], freeSets } };
};
