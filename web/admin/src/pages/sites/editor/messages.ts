/**
 * The page editor's field messages: every block field's label, help line,
 * choice names and examples, the styling sections and the field controls' own
 * words.
 *
 * They live beside the editor rather than in `src/i18n/catalogue.ts` because
 * the English catalogue is part of the first paint, and these 200-odd strings
 * would take that payload over its budget (`perf/budget.json`, `PERF.md`).
 * English is here, in its own chunk; every other locale is its own chunk under
 * `locales/`, loaded through [`EDITOR_LOADERS`] like the main catalogue's
 * `CATALOGUE_LOADERS`. The guarantees are the main catalogue's: each table is
 * typed against `en`, so a key added in English and missing elsewhere is a
 * `tsc` error, a locale with no loader is a `tsc` error, and
 * `src/generated/blocks.json` names only keys this table holds
 * (`parseBlockRegistry`).
 */
import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { useLocale } from '../../../i18n';
import type { Locale, MessageParams } from '../../../i18n';

export const en = {
  'blockField.media.attachments': 'Items to show',
  'fieldHelp.media.attachments': 'Each item shows its main media, or the media you pick.',
  'blockField.media.layout': 'Arrange items',
  'fieldHelp.media.layout': 'How several items line up on the page.',
  'blockChoice.vertical': 'Stacked',
  'blockChoice.horizontal': 'Side by side',
  'blockField.media.media_display': 'Show media as',
  'fieldHelp.media.media_display': 'A thumbnail is small and links to the full media.',
  'blockChoice.embed': 'Full media (player or image)',
  'blockChoice.thumbnail': 'Thumbnail',
  'blockField.media.thumbnail_type': 'Thumbnail size',
  'fieldHelp.media.thumbnail_type': 'Square crops the image; the others keep its shape.',
  'blockChoice.size_large': 'Large',
  'blockChoice.size_medium': 'Medium',
  'blockChoice.size_square': 'Square',
  'blockField.media.show_title_option': 'Title above each item',
  'fieldHelp.media.show_title_option': 'The heading above each item.',
  'blockChoice.item_title': 'Item title',
  'blockChoice.file_name': 'Media title',
  'blockChoice.no_title': 'No title',
  'blockField.browsePreview.resource_type': 'What to list',
  'fieldHelp.browsePreview.resource_type': 'Which kind of resource the block lists.',
  'blockChoice.items': 'Items',
  'blockChoice.item_sets': 'Item sets',
  'blockChoice.media': 'Media',
  'blockField.browsePreview.query': 'Filter',
  'fieldHelp.browsePreview.query': 'Only matching resources of this site are listed.',
  'blockField.browsePreview.limit': 'How many',
  'fieldHelp.browsePreview.limit': 'The most results the block shows.',
  'blockField.browsePreview.components': 'Show in each result',
  'fieldHelp.browsePreview.components': 'What each result shows.',
  'blockChoice.resource-heading': 'Title',
  'blockChoice.resource-body': 'Description',
  'blockField.browsePreview.heading': 'Heading',
  'fieldHelp.browsePreview.heading': 'Shown above the list. Empty shows none.',
  'fieldExample.browsePreview.heading': 'e.g. Recently added',
  'blockField.browsePreview.link-text': 'Link to all results',
  'fieldHelp.browsePreview.link-text': 'A link to the full list. Empty shows none.',
  'fieldExample.browsePreview.link-text': 'e.g. Browse all',
  'blockField.listOfSites.sort': 'Order',
  'fieldHelp.listOfSites.sort': 'The order of the sites.',
  'blockChoice.alpha': 'Alphabetical',
  'blockChoice.oldest': 'Oldest first',
  'blockChoice.newest': 'Newest first',
  'blockField.listOfSites.limit': 'How many sites',
  'fieldHelp.listOfSites.limit': '0 lists every site.',
  'blockField.listOfSites.pagination': 'Split into pages',
  'fieldHelp.listOfSites.pagination': 'Shows that many per page, with page links.',
  'blockField.listOfSites.summaries': 'Show summaries',
  'fieldHelp.listOfSites.summaries': 'Each site’s summary under its title.',
  'blockField.listOfSites.thumbnails': 'Show thumbnails',
  'fieldHelp.listOfSites.thumbnails': 'Each site’s thumbnail image.',
  'blockField.listOfSites.exclude_current': 'Leave out this site',
  'fieldHelp.listOfSites.exclude_current': 'This site does not list itself.',
  'blockField.tableOfContents.depth': 'Levels of sub-pages',
  'fieldHelp.tableOfContents.depth': '1 lists only the direct sub-pages.',
  'blockField.lineBreak.break_type': 'Style',
  'fieldHelp.lineBreak.break_type': 'Space only adds room; a line draws a rule.',
  'blockChoice.transparent': 'Space only',
  'blockChoice.opaque': 'Visible line',
  'blockField.itemWithMetadata.attachments': 'Items to show',
  'fieldHelp.itemWithMetadata.attachments': 'Each item shows its media and metadata.',
  'blockField.pageDateTime.display': 'Dates shown',
  'fieldHelp.pageDateTime.display': 'Which dates of this page visitors see.',
  'blockChoice.created_modified': 'Created and modified',
  'blockChoice.created': 'Created',
  'blockChoice.modified': 'Modified',
  'blockField.pageDateTime.date_format': 'Date style',
  'fieldHelp.pageDateTime.date_format': 'From short (1/2/26) to full (with weekday).',
  'blockChoice.date_none': 'None',
  'blockChoice.date_short': 'Short',
  'blockChoice.date_medium': 'Medium',
  'blockChoice.date_long': 'Long',
  'blockChoice.date_full': 'Full',
  'blockField.pageDateTime.time_format': 'Time style',
  'fieldHelp.pageDateTime.time_format': 'None hides the time.',
  'blockField.blockGroup.span': 'Blocks in the group',
  'fieldHelp.blockGroup.span': 'How many blocks after this one the group holds.',
  'blockField.iiifPresentation.manifest_url': 'Manifest URL',
  'fieldHelp.iiifPresentation.manifest_url': 'The address of a IIIF manifest.',
  'fieldExample.url': 'https://…',
  'blockField.iiifPresentation.title': 'Title',
  'fieldHelp.iiif.title': 'Shown above the viewer when turned on below.',
  'blockField.iiifPresentation.show_title': 'Show title',
  'fieldHelp.iiif.show_title': 'Prints the title above the viewer.',
  'blockField.iiifImage.image_url': 'Image URL',
  'fieldHelp.iiifImage.image_url': 'The address of a IIIF image.',
  'blockField.iiifImage.title': 'Title',
  'blockField.iiifImage.show_title': 'Show title',
  'blockField.asset.attachments': 'Images',
  'fieldHelp.asset.attachments': 'Each image can link to a page.',
  'blockField.asset.id': 'Image',
  'fieldHelp.asset.id': 'An uploaded image file.',
  'blockField.asset.page': 'Links to page',
  'fieldHelp.asset.page': 'Clicking the image opens this page.',
  'blockField.asset.alt_link_title': 'Link text',
  'fieldHelp.asset.alt_link_title': 'Replaces the page title under the image.',
  'blockField.asset.caption': 'Caption',
  'fieldHelp.asset.caption': 'Shown under the image.',
  'blockField.html.html': 'HTML',
  'fieldHelp.html.html': 'Text, images and links, shown as you format them.',
  'blockField.listOfPages.pagelist': 'Pages',
  'fieldHelp.listOfPages.pagelist': 'The pages listed, in order.',
  'blockField.oembed.url': 'Media URL',
  'fieldHelp.oembed.url': 'A link from YouTube, Vimeo or another oEmbed provider.',
  'blockField.oembed.refresh': 'Fetch the embed again',
  'fieldHelp.oembed.refresh': 'Reloads the embed from the provider on save.',
  'romekaField.hero.heading': 'Heading',
  'fieldHelp.hero.heading': 'The large text on the banner.',
  'fieldExample.hero.heading': 'e.g. Welcome to the collection',
  'romekaField.hero.subheading': 'Subheading',
  'fieldHelp.hero.subheading': 'One line under the heading.',
  'romekaField.hero.body': 'Text',
  'fieldHelp.hero.body': 'Optional paragraphs under the subheading.',
  'romekaField.hero.background': 'Background image',
  'fieldHelp.hero.background': 'Fills the banner behind the text.',
  'romekaField.hero.overlay': 'Darken the image',
  'fieldHelp.hero.overlay': 'Darkens the image so the text stays readable.',
  'romekaChoice.none': 'None',
  'romekaChoice.light': 'Light',
  'romekaChoice.medium': 'Medium',
  'romekaChoice.strong': 'Strong',
  'romekaField.hero.alignment': 'Text position',
  'fieldHelp.hero.alignment': 'Where the text sits on the banner.',
  'blockChoice.left': 'Left',
  'blockChoice.center': 'Center',
  'blockChoice.right': 'Right',
  'romekaField.hero.cta_label': 'Button text',
  'fieldHelp.hero.cta_label': 'The button shows when it has text and a link.',
  'fieldExample.hero.cta_label': 'e.g. Explore',
  'romekaField.hero.cta_page': 'Button links to',
  'fieldHelp.hero.cta_page': 'A page of this site, or any web address.',
  'blockLayoutData.padding': 'Padding',
  'fieldHelp.layout.padding': 'Space inside the block’s edge. A number alone is pixels.',
  'blockLayoutData.max_width': 'Maximum width',
  'fieldHelp.layout.length': 'A number alone is pixels; % and rem also work.',
  'fieldExample.layout.max_width': 'e.g. 640px or 80%',
  'blockLayoutData.min_height': 'Minimum height',
  'fieldExample.layout.min_height': 'e.g. 300px',
  'blockLayoutData.background_color': 'Background color',
  'fieldHelp.layout.background_color': 'Fills the block behind its content.',
  'blockLayoutData.background_image_asset': 'Background image',
  'fieldHelp.layout.background_image_asset': 'Shown behind the block’s content.',
  'blockLayoutData.background_image_size': 'Image fit',
  'fieldHelp.layout.background_image_size': 'Fill may crop the image; whole may leave gaps.',
  'blockChoice.cover': 'Fill the block',
  'blockChoice.contain': 'Show the whole image',
  'blockLayoutData.background_image_position_y': 'Image anchor (vertical)',
  'fieldHelp.layout.background_image_position': 'Which part of the image stays in view.',
  'blockChoice.top': 'Top',
  'blockChoice.bottom': 'Bottom',
  'blockLayoutData.background_image_position_x': 'Image anchor (horizontal)',
  'blockLayoutData.alignment_block': 'Block position',
  'fieldHelp.layout.alignment_block': 'Where a narrow block sits across the page.',
  'blockLayoutData.alignment_text': 'Text alignment',
  'fieldHelp.layout.alignment_text': 'How the lines of text line up.',
  'blockChoice.justify': 'Justify',
  'blockLayoutData.grid_column_position': 'Starts at column',
  'fieldHelp.layout.grid_column_position': 'A column number, or auto to follow the block before.',
  'fieldExample.layout.grid_column_position': 'auto',
  'blockLayoutData.grid_column_span': 'Columns wide',
  'fieldHelp.layout.grid_column_span': 'How many grid columns the block covers.',
  'blockLayoutData.template_name': 'Template',
  'fieldHelp.layout.template_name': 'A template file from the theme, by name.',
  'blockLayoutData.class': 'CSS class',
  'fieldHelp.layout.class': 'Class names for custom CSS, separated by spaces.',
  'pageLayoutData.template_name': 'Template',
  'pageLayoutData.class': 'Class',
  'pageLayoutData.grid_columns': 'Columns',
  'fieldHelp.page.grid_columns': 'How many columns the grid has.',
  'pageLayoutData.grid_column_gap': 'Column gap (px)',
  'fieldHelp.page.grid_gap': 'Space between blocks, in pixels.',
  'pageLayoutData.grid_row_gap': 'Row gap (px)',
  'layoutGroup.spacing': 'Spacing',
  'layoutGroup.size': 'Size',
  'layoutGroup.background': 'Background',
  'layoutGroup.alignment': 'Alignment',
  'layoutGroup.grid': 'Grid position',
  'layoutGroup.advanced': 'Advanced',
  'layoutGroup.gridOnly': 'Only on grid pages.',
  'sides.linked': 'Same on all sides',
  'sides.all': 'All sides',
  'sides.presets': 'Presets',
  'sides.none': 'None',
  'sides.small': 'Small',
  'sides.medium': 'Medium',
  'sides.large': 'Large',
  'color.picker': 'Choose a color',
  'color.clear': 'No color',
  'color.invalid': 'Use # and 3 or 6 hex digits.',
  'link.page': 'A page',
  'link.url': 'A web address',
  'layoutData.legend': 'Block styling',
  'query.itemSetCount': 'Item sets in this site that match: {count}.',
  'query.mediaCount': 'Media in this site that match: {count}.',
  'attachments.mainMedia': 'Main media',
  'attachments.pick': 'Add or remove items',
  'layoutData.pageLegend': 'Page layout',
  'layoutGroup.pageGrid': 'Grid',
  'choice.unset': 'Not set',
};

/** One page editor message id. */
export type EditorMessageKey = keyof typeof en;

/** A page editor message table; every locale's is typed against English. */
export type EditorMessages = Readonly<Record<EditorMessageKey, string>>;

/** One loader per locale, as `CATALOGUE_LOADERS` has. */
const EDITOR_LOADERS: Readonly<Record<Locale, () => Promise<EditorMessages>>> = {
  en: () => Promise.resolve(en),
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
};

const loaded = new Map<Locale, EditorMessages>([['en', en]]);

/**
 * The active locale's table, or `null` until the first one arrives. A later
 * locale switch keeps the previous table until the new one loads, as the main
 * store keeps its translator, so the editor never unmounts a draft to wait.
 */
export const useEditorMessages = (): EditorMessages | null => {
  const locale = useLocale();
  const [table, setTable] = useState<EditorMessages | null>(() => loaded.get(locale) ?? null);
  useEffect(() => {
    const cached = loaded.get(locale);
    if (cached !== undefined) {
      setTable(cached);
      return undefined;
    }
    let current = true;
    void EDITOR_LOADERS[locale]().then((messages) => {
      loaded.set(locale, messages);
      if (current) {
        setTable(messages);
      }
    });
    return () => {
      current = false;
    };
  }, [locale]);
  return table;
};

/** The table the editor renders with. English outside a provider, as in a component test. */
export const EditorMessagesContext = createContext<EditorMessages>(en);

/** Every page editor message id. */
export const EDITOR_MESSAGE_KEYS: ReadonlySet<string> = new Set(Object.keys(en));

/** Whether a string is a page editor message id. */
export const isEditorMessageKey = (value: string): value is EditorMessageKey =>
  EDITOR_MESSAGE_KEYS.has(value);

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The editor's message lookup in the table [`EditorMessagesContext`] provides. */
export const useEditorText = (): ((key: EditorMessageKey, params?: MessageParams) => string) => {
  const messages = useContext(EditorMessagesContext);
  return useMemo(
    () =>
      (key: EditorMessageKey, params?: MessageParams): string =>
        interpolate(messages[key], params),
    [messages],
  );
};
