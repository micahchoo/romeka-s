/**
 * The two parts of the site frame that every page shares and the page editor
 * can select: the header (with the banner above it) and the footer. The
 * preview render marks them with `data-romeka-region`
 * (`crates/omeka-site/src/view.rs::SiteRegion`); the Theme settings behind
 * them are site-wide, not the page's.
 */

export const SITE_REGIONS = ['header', 'footer'] as const;

export type SiteRegion = (typeof SITE_REGIONS)[number];

/** The region a `data-romeka-region` value names, or `null` for any other text. */
export const parseRegion = (value: string | null): SiteRegion | null =>
  SITE_REGIONS.find((region) => region === value) ?? null;

/** The label the overlay, the crumb and the inspector heading show. */
export const REGION_LABEL = {
  header: 'region.header',
  footer: 'region.footer',
} as const satisfies Readonly<Record<SiteRegion, string>>;
