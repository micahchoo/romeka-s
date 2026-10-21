/**
 * Boundary-parser tests. The fixtures are byte-for-byte captures of the
 * shapes documented in `application/src/Api/Representation/*.php` of
 * omeka-s at 5e413f0, so these tests freeze the wire contract the client
 * depends on.
 */
import { describe, expect, it } from 'vitest';
import { parseLinkHeader } from '../src/api/client';
import { loadFixture } from './loadFixture';
import {
  ParseError,
  classIdFromParam,
  isRecord,
  isUnknownArray,
  itemIdFromParam,
  mediaIdFromParam,
  parseApiErrors,
  parseItem,
  parseListOf,
  parseMedia,
  parseResourceClass,
  parseUser,
  propertyIdFromParam,
  templateIdFromParam,
  vocabularyIdFromParam,
} from '../src/api/parse';

const at = <T>(values: readonly T[], index: number): T => {
  const value = values[index];
  if (value === undefined) {
    throw new Error(`expected a fixture entry at index ${String(index)}`);
  }
  return value;
};

const parseErrorOf = (parse: () => unknown): ParseError => {
  try {
    parse();
  } catch (error) {
    if (error instanceof ParseError) {
      return error;
    }
    throw error;
  }
  throw new Error('expected the parser to throw');
};

describe('parseItem', () => {
  it('item_json_ld_fields_map_to_typed_values', () => {
    const item = parseItem(loadFixture('item-1.json'));
    expect(item.id).toBe(1);
    expect(item.title).toBe('Fixture Item One');
    expect(item.isPublic).toBe(false);
    expect(item.owner?.id).toBe(3);
    expect(item.resourceClass?.id).toBe(13);
    expect(item.resourceTemplate?.id).toBe(2);
    expect(item.media.map((entry) => entry.id)).toEqual([7, 8]);
    expect(item.primaryMedia?.id).toBe(7);
    expect(item.itemSets.map((entry) => entry.id)).toEqual([4]);
    expect(item.sites).toEqual([]);
    expect(item.created).toBe('2026-10-16T09:30:00+00:00');
    expect(item.modified).toBe('2026-10-16T10:15:00+00:00');
    expect(item.thumbnailDisplayUrls.square).toBe('http://localhost/files/square/1234abc.jpg');
  });

  it('value_groups_are_keyed_by_property_term', () => {
    const item = parseItem(loadFixture('item-1.json'));

    const title = at(item.values.get('dcterms:title') ?? [], 0);
    expect(title).toMatchObject({
      kind: 'literal',
      text: 'Fixture Item One',
      lang: 'en',
      propertyLabel: 'Title',
    });

    const relation = at(item.values.get('dcterms:relation') ?? [], 0);
    expect(relation).toMatchObject({
      kind: 'resource',
      resourceId: 9,
      resourceName: 'items',
      displayTitle: 'Fixture Item Two',
      url: null,
    });

    const subject = at(item.values.get('dcterms:subject') ?? [], 0);
    expect(subject).toMatchObject({
      kind: 'uri',
      uri: 'http://example.org/subjects/fixtures',
      label: 'Fixtures',
    });
  });

  it('parse_error_names_the_failing_json_path', () => {
    const body = loadFixture('item-1.json');
    if (!isRecord(body)) {
      throw new Error('fixture item is not an object');
    }
    const titles = body['dcterms:title'];
    if (!isUnknownArray(titles)) {
      throw new Error('fixture item has no dcterms:title group');
    }
    const firstTitle = titles[0];
    if (!isRecord(firstTitle)) {
      throw new Error('fixture item has no first dcterms:title value');
    }
    firstTitle['property_id'] = 'one';

    const error = parseErrorOf(() => parseItem(body));
    expect(error.path).toEqual(['dcterms:title', 0, 'property_id']);
    expect(error.message).toBe('$.dcterms:title[0].property_id: expected number');
  });

  it('parse_value_reads_a_module_data_type_by_its_shape', () => {
    const body = loadFixture('item-1.json');
    if (!isRecord(body)) {
      throw new Error('fixture item is not an object');
    }
    const base = { property_id: 1, property_label: 'Title', is_public: true };
    const item = parseItem({
      ...body,
      'dcterms:title': [
        { ...base, type: 'customvocab:3', '@value': 'Red', '@language': 'en' },
        { ...base, type: 'customvocab:4', '@id': 'http://example.org/red', 'o:label': 'Red' },
        { ...base, type: 'numeric:timestamp' },
      ],
    });
    const [literal, uri, other] = item.values.get('dcterms:title') ?? [];
    expect(literal).toMatchObject({ kind: 'literal', type: 'customvocab:3', text: 'Red', lang: 'en' });
    expect(uri).toMatchObject({ kind: 'uri', type: 'customvocab:4', uri: 'http://example.org/red' });
    expect(other).toMatchObject({ kind: 'other', type: 'numeric:timestamp' });
  });

  it('parse_item_rejects_a_non_object_root', () => {
    const error = parseErrorOf(() => parseItem(null));
    expect(error.path).toEqual([]);
    expect(error.message).toBe('$: expected object');
  });
});

describe('parseMedia', () => {
  const page = parseListOf(parseMedia)(loadFixture('media-page.json'));

  it('media_upload_keeps_source_size_and_thumbnail_urls', () => {
    const photo = at(page, 0);
    expect(photo.id).toBe(7);
    expect(photo.title).toBe('Fixture photo');
    expect(photo.ingester).toBe('upload');
    expect(photo.renderer).toBe('image');
    expect(photo.source).toBe('photo.jpg');
    expect(photo.mediaType).toBe('image/jpeg');
    expect(photo.size).toBe(20480);
    expect(photo.thumbnailUrls.medium).toBe('http://localhost/files/medium/photo.jpg');
    expect(photo.item?.id).toBe(1);
  });

  it('media_without_thumbnails_parses_empty_thumbnail_urls', () => {
    const renderer = at(page, 1);
    expect(renderer.id).toBe(8);
    expect(renderer.title).toBeNull();
    expect(renderer.ingester).toBe('html');
    expect(renderer.renderer).toBe('html');
    expect(renderer.thumbnailUrls).toEqual({});
  });

  it('media_empty_thumbnail_url_list_parses_as_no_thumbnails', () => {
    // `media-951.json` is a byte-for-byte capture of `GET /api/media/951` on the
    // running port, where `o:thumbnail_urls` is the empty PHP array.
    const media = parseMedia(loadFixture('media-951.json'));
    expect(media.id).toBe(951);
    expect(media.title).toBeNull();
    expect(media.source).toBe('BHC006_GMT_AF_010a.jpg');
    expect(media.filename).toBe('e82cef00da75e9c5661a9ee66155ef419cf89988.jpg');
    expect(media.size).toBe(5989397);
    expect(media.thumbnailUrls).toEqual({});
    expect(media.originalUrl).toBe(
      'http://localhost:8124/files/original/e82cef00da75e9c5661a9ee66155ef419cf89988.jpg',
    );
  });

  it('media_access_copy_member_parses_its_url_and_state', () => {
    // `o-module-romeka:access` is the port's member; that the term is not read
    // as a property value is the contract (an older server sends none).
    const body = loadFixture('media-page.json');
    if (!isUnknownArray(body)) {
      throw new Error('expected media-page.json to be an array');
    }
    const entry = body[0];
    if (!isRecord(entry)) {
      throw new Error('expected the media to be an object');
    }
    entry['o-module-romeka:access'] = {
      state: 'ready',
      url: 'http://localhost/files/access/abc.mp4',
      media_type: 'video/mp4',
    };

    const media = parseMedia(entry);
    expect(media.access).toEqual({
      state: 'ready',
      reason: null,
      progress: null,
      url: 'http://localhost/files/access/abc.mp4',
      mediaType: 'video/mp4',
    });
    expect(media.values.has('o-module-romeka:access')).toBe(false);
  });

  it('media_without_an_access_member_reads_none', () => {
    const media = parseMedia(loadFixture('media-951.json'));
    expect(media.access).toBeNull();
  });

  it('media_html_content_members_are_not_read_as_values', () => {
    // `Module::filterHtmlMediaJsonLd` adds `o-cnt:chars` and
    // `o-cnt:characterEncoding` to an html media beside the values, as strings.
    const body = loadFixture('media-page.json');
    if (!isUnknownArray(body)) {
      throw new Error('expected media-page.json to be an array');
    }
    const entry = body[1];
    if (!isRecord(entry)) {
      throw new Error('expected the html media to be an object');
    }
    entry['o-cnt:chars'] = 'A note in HTML';
    entry['o-cnt:characterEncoding'] = 'UTF-8';

    const media = parseMedia(entry);
    expect(media.ingester).toBe('html');
    expect(media.renderer).toBe('html');
    expect(media.source).toBeNull();
  });

  it('media_rejects_a_non_empty_thumbnail_url_list', () => {
    const body = loadFixture('media-951.json');
    if (!isRecord(body)) {
      throw new Error('fixture media is not an object');
    }
    body['o:thumbnail_urls'] = ['http://localhost/files/square/abcdef.jpg'];

    const error = parseErrorOf(() => parseMedia(body));
    expect(error.path).toEqual(['o:thumbnail_urls']);
    expect(error.message).toBe('$.o:thumbnail_urls: expected an empty array or an object');
  });
});

describe('parseUser', () => {
  it('user_created_is_a_bare_iso_string', () => {
    const users = parseListOf(parseUser)(loadFixture('users-page.json'));
    expect(users.map((user) => user.name)).toEqual(['Ada Lovelace', 'Grace Hopper']);
    const ada = at(users, 0);
    expect(ada.id).toBe(3);
    expect(ada.created).toBe('2026-10-15T08:00:00+00:00');
    expect(ada.role).toBe('global_admin');
    expect(ada.isActive).toBe(true);
    expect(at(users, 1).isActive).toBe(false);
  });
});

describe('parseResourceClass', () => {
  it('resource_class_term_label_and_vocabulary_parse', () => {
    const classes = parseListOf(parseResourceClass)(loadFixture('resource-classes-page.json'));
    const stillImage = at(classes, 0);
    expect(stillImage.id).toBe(13);
    expect(stillImage.term).toBe('dctype:StillImage');
    expect(stillImage.localName).toBe('StillImage');
    expect(stillImage.label).toBe('Still Image');
    expect(stillImage.comment).toBeNull();
    expect(stillImage.vocabulary.id).toBe(2);
    expect(at(classes, 1).term).toBe('bibo:Book');
  });
});

describe('parseLinkHeader', () => {
  it('link_header_yields_first_next_and_last_pages', () => {
    const header =
      '<http://localhost/api/items?page=1>; rel="first", ' +
      '<http://localhost/api/items?page=2&per_page=25>; rel="next", ' +
      '<http://localhost/api/items?page=3&per_page=25>; rel="last"';
    const links = parseLinkHeader(header);
    expect(links.get('first')).toBe(1);
    expect(links.get('next')).toBe(2);
    expect(links.get('last')).toBe(3);
    expect(links.has('prev')).toBe(false);
    expect(parseLinkHeader(null).size).toBe(0);
  });
});

describe('parseApiErrors', () => {
  it('api_error_body_maps_string_and_array_messages', () => {
    expect(parseApiErrors({ errors: { error: 'Item not found' } })).toEqual({
      error: ['Item not found'],
    });
    expect(parseApiErrors({ errors: { 'o:name': ['Name is required'] } })).toEqual({
      'o:name': ['Name is required'],
    });
    expect(parseApiErrors({ errors: 'oops' })).toBeNull();
    expect(parseApiErrors(null)).toBeNull();
  });
});

describe('itemIdFromParam', () => {
  it('item_id_param_accepts_positive_integers_only', () => {
    expect(itemIdFromParam('12')).toBe(12);
    expect(itemIdFromParam('0')).toBeNull();
    expect(itemIdFromParam('-3')).toBeNull();
    expect(itemIdFromParam('abc')).toBeNull();
    expect(itemIdFromParam('')).toBeNull();
  });
});

describe('mediaIdFromParam', () => {
  it('media_id_param_accepts_positive_integers_only', () => {
    expect(mediaIdFromParam('951')).toBe(951);
    expect(mediaIdFromParam('0')).toBeNull();
    expect(mediaIdFromParam('-3')).toBeNull();
    expect(mediaIdFromParam('1.5')).toBeNull();
    expect(mediaIdFromParam('abc')).toBeNull();
    expect(mediaIdFromParam('')).toBeNull();
  });
});

describe('vocabulary family id params', () => {
  it('vocabulary_property_class_and_template_params_accept_positive_integers_only', () => {
    expect(vocabularyIdFromParam('3')).toBe(3);
    expect(propertyIdFromParam('4')).toBe(4);
    expect(classIdFromParam('13')).toBe(13);
    expect(templateIdFromParam('2')).toBe(2);

    for (const parse of [vocabularyIdFromParam, propertyIdFromParam, classIdFromParam, templateIdFromParam]) {
      expect(parse('0')).toBeNull();
      expect(parse('-3')).toBeNull();
      expect(parse('1.5')).toBeNull();
      expect(parse('abc')).toBeNull();
      expect(parse('')).toBeNull();
    }
  });
});
