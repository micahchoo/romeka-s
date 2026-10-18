/**
 * The block layouts a page can use. There is no API endpoint that lists them,
 * so the names and catalogue keys are hardcoded from the port's registry. The
 * names are the fifteen registered in `crates/omeka-site/src/blocks/mod.rs`
 * (which mirrors `application/config/module.config.php:805-825` of omeka-s at
 * 5e413f0); the labels are `BlockLayout::label` in
 * `crates/omeka-core/src/site.rs`.
 */
import type { MessageKey } from '../../i18n';

export interface BlockLayoutOption {
  readonly name: string;
  readonly labelKey: MessageKey;
}

export const BLOCK_LAYOUTS: readonly BlockLayoutOption[] = [
  { name: 'pageTitle', labelKey: 'layout.pageTitle' },
  { name: 'media', labelKey: 'layout.media' },
  { name: 'browsePreview', labelKey: 'layout.browsePreview' },
  { name: 'listOfSites', labelKey: 'layout.listOfSites' },
  { name: 'tableOfContents', labelKey: 'layout.tableOfContents' },
  { name: 'lineBreak', labelKey: 'layout.lineBreak' },
  { name: 'itemWithMetadata', labelKey: 'layout.itemWithMetadata' },
  { name: 'pageDateTime', labelKey: 'layout.pageDateTime' },
  { name: 'blockGroup', labelKey: 'layout.blockGroup' },
  { name: 'iiifPresentation', labelKey: 'layout.iiifPresentation' },
  { name: 'iiifImage', labelKey: 'layout.iiifImage' },
  { name: 'asset', labelKey: 'layout.asset' },
  { name: 'html', labelKey: 'layout.html' },
  { name: 'listOfPages', labelKey: 'layout.listOfPages' },
  { name: 'oembed', labelKey: 'layout.oembed' },
];

/**
 * The options for one block, keeping a stored layout the registry does not
 * register as its own entry so an unknown layout is never silently switched.
 */
export const blockLayoutOptions = (current: string): readonly BlockLayoutOption[] =>
  BLOCK_LAYOUTS.some((option) => option.name === current)
    ? BLOCK_LAYOUTS
    : [...BLOCK_LAYOUTS, { name: current, labelKey: 'layout.unknown' }];
