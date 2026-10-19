/**
 * The Standards page's routes: `#/standards/<tab>`, then `new` or a member id.
 * Every link and redirect into the vocabulary family goes through here, so the
 * tab names live in one place.
 */
export type StandardsTab = 'vocabularies' | 'properties' | 'classes';

export const STANDARDS_TABS: readonly StandardsTab[] = ['vocabularies', 'properties', 'classes'];

export const isStandardsTab = (value: string): value is StandardsTab =>
  STANDARDS_TABS.some((tab) => tab === value);

export const standardsHref = (tab: StandardsTab, target?: number | 'new'): string =>
  target === undefined ? `#/standards/${tab}` : `#/standards/${tab}/${String(target)}`;
