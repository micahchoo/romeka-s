/**
 * The seven steps of an import, in order. A step's slug is its address
 * (`#/import/<id>/<slug>`) and its title key the tab's text.
 */
import type { ImportDetail, ImportId, ImportPlan } from '../../api/imports';
import type { ImportMessageKey } from './messages';

export const TABS = [
  { slug: 'folder', titleKey: 'archive.tab.folder' },
  { slug: 'sets', titleKey: 'archive.tab.sets' },
  { slug: 'types', titleKey: 'archive.tab.types' },
  { slug: 'sources', titleKey: 'archive.tab.sources' },
  { slug: 'grid', titleKey: 'archive.tab.grid' },
  { slug: 'check', titleKey: 'archive.tab.check' },
  { slug: 'run', titleKey: 'archive.tab.run' },
] as const satisfies readonly { readonly slug: string; readonly titleKey: ImportMessageKey }[];

export type TabSlug = (typeof TABS)[number]['slug'];

export const tabFromSlug = (slug: string | undefined): TabSlug | null =>
  slug === undefined ? 'folder' : (TABS.find((tab) => tab.slug === slug)?.slug ?? null);

export const tabHref = (id: ImportId, slug: TabSlug): string => `#/import/${String(id)}/${slug}`;

/** The Grid step showing only the rows with problems, as the Check step links to it. */
export const PROBLEMS_VIEW = 'problems';
export const gridProblemsHref = (id: ImportId): string => `${tabHref(id, 'grid')}/${PROBLEMS_VIEW}`;

/** What every step receives: the import, its plan as edited, and the way to change it. */
export interface TabProps {
  readonly detail: ImportDetail;
  readonly plan: ImportPlan;
  /** Whether the plan may still change: only a draft's may. */
  readonly editable: boolean;
  readonly onChange: (change: (plan: ImportPlan) => ImportPlan) => void;
  /** Hands the page a newer copy of the import, after a run starts or a poll answers. */
  readonly onDetail: (detail: ImportDetail) => void;
  /** Whether the address asked for the rows with problems only (`gridProblemsHref`). */
  readonly problemsOnly: boolean;
}
