/**
 * Media parser tests. `media-page.json` and `media-951.json` are byte-for-byte
 * captures of the wire shapes documented in
 * `application/src/Api/Representation/MediaRepresentation.php` of omeka-s at
 * 5e413f0, so these tests freeze the contract the client depends on.
 */
import { describe, expect, it } from 'vitest';
import { parseMedia } from '../src/api/media';
import { ParseError, isRecord, isUnknownArray, parseListOf } from '../src/api/parse';
import { loadFixture } from './loadFixture';

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
  throw new Error('expected a ParseError');
};

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
