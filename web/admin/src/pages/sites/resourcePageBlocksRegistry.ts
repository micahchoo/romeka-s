/**
 * The generated resource page block registry, narrowed once at module load.
 *
 * `src/generated/resourcePageBlocks.json` is written by
 * `crates/omeka-site/src/resource_page_schema.rs` from the layouts
 * `resource_page.rs` registers, and a Rust test keeps it byte-identical. The
 * parser rejects a `labelKey` that is not in the catalogue.
 */
import raw from '../../generated/resourcePageBlocks.json';
import { parseResourcePageBlocksRegistry } from './resourcePageBlocksSchema';
import type { ResourcePageBlocksRegistry } from './resourcePageBlocksSchema';

export const RESOURCE_PAGE_BLOCKS_REGISTRY: ResourcePageBlocksRegistry =
  parseResourcePageBlocksRegistry(raw);
