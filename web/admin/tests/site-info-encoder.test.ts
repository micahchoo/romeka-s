/** The Settings form's partial update body, as `SiteAdapter::hydrate` reads it. */
import { describe, expect, it } from 'vitest';
import { encodeSiteHomepage, encodeSiteInfo } from '../src/api/write';
import type { SiteInfoWrite } from '../src/api/write';
import { pageIdFromParam, userIdFromParam } from '../src/api/parse';

const must = <T>(id: T | null): T => {
  if (id === null) {
    throw new Error('the id did not parse');
  }
  return id;
};

const info: SiteInfoWrite = {
  title: '  My site ',
  slug: ' my-site ',
  theme: 'default',
  summary: '   ',
  owner: null,
  isPublic: true,
};

describe('encodeSiteInfo', () => {
  it('trims text, turns a blank summary into null and leaves the owner and sync alone', () => {
    expect(encodeSiteInfo(info)).toEqual({
      'o:title': 'My site',
      'o:slug': 'my-site',
      'o:theme': 'default',
      'o:summary': null,
      'o:is_public': true,
    });
  });

  it('names the owner and keeps a summary when both are set', () => {
    const body = encodeSiteInfo({ ...info, summary: ' Hello ', owner: must(userIdFromParam('7')) });
    expect(body['o:summary']).toBe('Hello');
    expect(body['o:owner']).toEqual({ 'o:id': 7 });
  });
});

describe('encodeSiteHomepage', () => {
  it('names a page, or sends null to hand the choice back to the menu', () => {
    expect(encodeSiteHomepage(must(pageIdFromParam('3')))).toEqual({ 'o:homepage': { 'o:id': 3 } });
    expect(encodeSiteHomepage(null)).toEqual({ 'o:homepage': null });
  });
});
