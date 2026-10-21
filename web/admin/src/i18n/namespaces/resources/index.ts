/**
 * The item, item set and media page messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `ResourcesMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'itemSites.legend': 'Sites',
  'itemSites.none': 'You cannot add items to any site.',
  'itemSites.createHelp':
    'If you choose no site, every site that keeps in sync with its search will add this item when it matches.',
  'loading.itemSet': 'Loading item set...',
  'loading.media': 'Loading media...',
  // Value tables.
  'values.none': 'No values.',
  'values.property': 'Property',
  'values.type': 'Type',
  'values.value': 'Value',
  'values.tableLabel': 'Values for {term}',
  'field.resourceTemplate': 'Resource template',
  'column.modified': 'Modified',
  // Resource pages.
  'items.add': 'Add item',
  'items.empty': 'No items found.',
  'items.mediaHeading': 'Media ({count})',
  'items.noMedia': 'No media.',
  'items.valuesHeading': 'Values',
  'items.rowName': 'Item #{id}',
  'items.editLabel': 'Edit this item',
  'items.deleteLabel': 'Delete this item',
  'items.noSets': 'No item sets.',
  'items.setsLegend': 'Item sets',
  'itemSets.open': 'Open item set',
  'itemSets.add': 'Add item set',
  'itemSets.deleteLabel': 'Delete this item set',
  'itemSets.rowName': 'Item set #{id}',
  'media.legend': 'Media',
  'media.altText': 'Alt text',
  'media.language': 'Language',
  'media.visibility': 'Visibility',
  'media.partOfItem': 'Part of item',
  'media.mimeType': 'MIME type',
  'media.renderer': 'Renderer',
  'media.ingester': 'Ingester',
  'media.source': 'Source',
  'media.filename': 'Filename',
  'media.size': 'Size',
  'media.sha256': 'SHA-256',
  'media.fileDerivatives': 'File derivatives',
  'media.editLabel': 'Edit this media',
  'media.deleteLabel': 'Delete this media',
  'media.altFallback': 'Media #{id}',
  'media.kind.image': 'Image',
  'media.kind.audio': 'Audio',
  'media.kind.video': 'Video',
  'media.kind.pdf': 'PDF',
  'media.kind.document': 'Document',
  'media.kind.model': '3D model',
  'media.kind.captions': 'Captions',
  'media.kind.embed': 'Embed',
  'media.kind.manifest': 'IIIF manifest',
  'media.kind.other': 'File',
  'media.browse.intro': 'Media belong to an item. To add one, open its item from',
  'media.browse.items': 'Items',
  'media.browse.batch': 'For many files, use',
  'media.browse.import': 'Import',
  'media.filter.label': 'Kind',
  'media.filter.all': 'All kinds',
  // Route parse errors.
  'route.invalidItemId': '"{param}" is not a valid item id.',
  'route.invalidItemSetId': '"{param}" is not a valid item set id.',
  'route.invalidMediaId': '"{param}" is not a valid media id.',
  'items.selectAll': 'Select all on this page',
  'items.selectItem': 'Select {title}',
  'items.selected': 'Selected: {count}',
  'items.bulkAdd': 'Add to site…',
  'items.bulkRemove': 'Remove from site…',
  'items.bulkSite': 'Site',
  'items.bulkChooseSite': 'Choose a site',
  'items.bulkConfirmAdd': 'Add {count} to {site}',
  'items.bulkDone': '{site} now has {total} items.',
  'items.bulkConfirmRemove': 'Remove {count} from {site}',
  'place.legend': 'Place on a map: {field}',
  'place.help': 'Click the map, or type "lat, lng". Each place is saved as a {term} value, which the site\'s maps show.',
  'place.mapLabel': 'Map: click to set the selected place',
  'place.value': 'Place {number}',
  'place.remove': 'Remove place {number}',
  'place.add': 'Add a place',
  'place.invalid': 'Type a latitude from -90 to 90, a comma, and a longitude from -180 to 180.',
  'place.unavailable': 'The map could not load. Type the place as "lat, lng".',
} as const;

/** A complete table of this namespace, in one locale. */
export type ResourcesMessages = Table<keyof typeof en>;

export const resources = defineNamespace('resources', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
