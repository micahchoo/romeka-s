/**
 * Parser tests for the wire types added in wave 3. Each fixture is the shape
 * frozen in `fixtures/api/` of omeka-s at 5e413f0, wrapped as a
 * list document so the tests freeze the contract the admin client depends on.
 */
import { describe, expect, it } from 'vitest';
import {
  ParseError,
  assetIdFromParam,
  jobIdFromParam,
  parseApiResource,
  parseAsset,
  parseDataType,
  parseItemSet,
  parseJob,
  parseListOf,
  parseModule,
  parseProperty,
  parseResourceTemplate,
  parseSite,
  parseVocabulary,
} from '../src/api/parse';
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
  throw new Error('expected the parser to throw');
};

describe('parseItemSet', () => {
  it('item_set_json_ld_fields_map_to_typed_values', () => {
    const itemSets = parseListOf(parseItemSet)(loadFixture('item-sets-page.json'));
    const itemSet = at(itemSets, 0);
    expect(itemSet.id).toBe(1);
    expect(itemSet.title).toBe('Featured photographs');
    expect(itemSet.isPublic).toBe(true);
    expect(itemSet.isOpen).toBe(true);
    expect(itemSet.owner?.id).toBe(3);
    expect(itemSet.itemsUrl).toBe('http://localhost/api/items?item_set_id=1');
    expect(itemSet.created).toBe('2024-02-10T08:15:00+00:00');
    expect(at(itemSets, 1).isOpen).toBe(false);
  });
});

describe('parseAsset', () => {
  it('asset_json_ld_fields_map_to_typed_values', () => {
    const assets = parseListOf(parseAsset)(loadFixture('assets-page.json'));
    const asset = at(assets, 0);
    expect(asset.id).toBe(1);
    expect(asset.name).toBe('omeka-logo.png');
    expect(asset.filename).toBe('a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.png');
    expect(asset.mediaType).toBe('image/png');
    expect(asset.assetUrl).toBe(
      'http://localhost/files/asset/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.png',
    );
    expect(asset.altText).toBeNull();
    expect(asset.owner?.id).toBe(3);
    expect(at(assets, 1).altText).toBe('A decorative heading');
  });
});

describe('parseJob', () => {
  it('job_json_ld_keeps_args_and_null_ended', () => {
    const jobs = parseListOf(parseJob)(loadFixture('jobs-page.json'));
    const job = at(jobs, 0);
    expect(job.id).toBe(1);
    expect(job.status).toBe('completed');
    expect(job.jobClass).toBe('Omeka\\Job\\UpdateSiteItems');
    expect(job.args).toEqual({ action: 'add', sites: [] });
    expect(job.started).toBe('2024-01-03T11:00:00+00:00');
    expect(job.ended).toBe('2024-01-03T11:00:05+00:00');
    expect(job.owner?.id).toBe(3);

    const running = at(jobs, 1);
    expect(running.args).toBeNull();
    expect(running.ended).toBeNull();
  });

  it('job_with_no_status_or_started_parses_as_null', () => {
    const jobs = parseListOf(parseJob)(loadFixture('job-no-status.json'));
    const job = at(jobs, 0);
    expect(job.status).toBeNull();
    expect(job.started).toBeNull();
  });
});

describe('parseModule', () => {
  it('module_id_is_the_directory_name_and_ini_reads_back', () => {
    const modules = parseListOf(parseModule)(loadFixture('modules-page.json'));
    const common = at(modules, 0);
    expect(common.id).toBe('Common');
    expect(common.state).toBe('active');
    expect(common.ini).toEqual({
      name: 'Common',
      version: '3.4.0',
      description: 'Provides common functionality for other modules.',
      author: 'Omeka Team',
      license: 'GPL-3.0-only',
      omeka_version_constraint: '^4.0.0',
    });
  });

  it('module_rejects_a_numeric_id', () => {
    const error = parseErrorOf(() => parseModule({ '@id': 'x', '@type': 'o:Module', 'o:id': 1, 'o:state': 'active', 'o:ini': {} }));
    expect(error.path).toEqual(['o:id']);
    expect(error.message).toBe('$.o:id: expected string');
  });
});

describe('parseProperty', () => {
  it('property_term_label_and_vocabulary_parse', () => {
    const properties = parseListOf(parseProperty)(loadFixture('properties-page.json'));
    const title = at(properties, 0);
    expect(title.id).toBe(1);
    expect(title.term).toBe('dcterms:title');
    expect(title.label).toBe('Title');
    expect(title.comment).toBe('A name given to the resource.');
    expect(title.vocabulary.id).toBe(1);
    expect(at(properties, 1).comment).toBeNull();
  });
});

describe('parseVocabulary', () => {
  it('vocabulary_prefix_namespace_and_label_parse', () => {
    const vocabularies = parseListOf(parseVocabulary)(loadFixture('vocabularies-page.json'));
    const dcterms = at(vocabularies, 0);
    expect(dcterms.id).toBe(1);
    expect(dcterms.prefix).toBe('dcterms');
    expect(dcterms.namespaceUri).toBe('http://purl.org/dc/terms/');
    expect(dcterms.label).toBe('Dublin Core');
    expect(dcterms.owner).toBeNull();
  });
});

describe('parseResourceTemplate', () => {
  it('resource_template_reads_each_property_entry', () => {
    const templates = parseListOf(parseResourceTemplate)(
      loadFixture('resource-templates-page.json'),
    );
    const template = at(templates, 0);
    expect(template.id).toBe(2);
    expect(template.label).toBe('Photograph');
    expect(template.owner?.id).toBe(3);
    expect(template.resourceClass?.id).toBe(13);
    expect(template.titleProperty?.id).toBe(1);
    expect(template.properties).toHaveLength(3);

    const [title, description, relation] = template.properties;
    expect(title?.dataTypes).toEqual(['literal']);
    expect(title?.isRequired).toBe(true);
    expect(description?.alternateLabel).toBe('Caption');
    expect(description?.defaultLang).toBe('en');
    expect(relation?.dataTypes).toEqual(['resource:item']);

    const empty = at(templates, 1);
    expect(empty.resourceClass).toBeNull();
    expect(empty.properties).toEqual([]);
  });
});

describe('malformed vocabulary family fixtures', () => {
  it('property_vocabulary_ref_fails_with_its_json_path', () => {
    const error = parseErrorOf(() => parseProperty(loadFixture('malformed-property.json')));
    expect(error.path).toEqual(['o:vocabulary']);
    expect(error.message).toBe('$.o:vocabulary: expected object');
  });
});

describe('parseDataType', () => {
  it('data_type_id_is_the_registered_name_in_registration_order', () => {
    const dataTypes = parseListOf(parseDataType)(loadFixture('data-types-page.json'));
    expect(dataTypes.map((dataType) => dataType.id)).toEqual([
      'literal',
      'uri',
      'resource',
      'resource:item',
      'resource:itemset',
      'resource:media',
    ]);
    expect(at(dataTypes, 0).label).toBe('Text');
    expect(at(dataTypes, 3).label).toBe('Item');
  });

  it('data_type_rejects_a_numeric_id', () => {
    const error = parseErrorOf(() => parseDataType(loadFixture('malformed-data-type.json')));
    expect(error.path).toEqual(['o:id']);
    expect(error.message).toBe('$.o:id: expected string');
  });

  it('asset_rejects_a_missing_asset_url', () => {
    const error = parseErrorOf(() => parseAsset(loadFixture('malformed-asset.json')));
    expect(error.path).toEqual(['o:asset_url']);
    expect(error.message).toBe('$.o:asset_url: expected a value');
  });
});

describe('the asset and job route ids', () => {
  it('asset_and_job_ids_parse_only_positive_integers', () => {
    expect(assetIdFromParam('12')).toBe(12);
    expect(jobIdFromParam('3')).toBe(3);
    expect(assetIdFromParam('')).toBeNull();
    expect(assetIdFromParam('0')).toBeNull();
    expect(jobIdFromParam('1.5')).toBeNull();
    expect(jobIdFromParam('abc')).toBeNull();
  });
});

describe('parseSite', () => {
  it('site_reads_slug_navigation_and_item_pool', () => {
    const sites = parseListOf(parseSite)(loadFixture('sites-page.json'));
    const site = at(sites, 0);
    expect(site.id).toBe(1);
    expect(site.slug).toBe('my-site');
    expect(site.theme).toBe('default');
    expect(site.title).toBe('My Site');
    expect(site.isPublic).toBe(true);
    expect(site.owner?.id).toBe(1);
    expect(site.pages.map((page) => page.id)).toEqual([1]);
    expect(site.navigation).toHaveLength(1);
    expect(site.siteItemsUrl).toBe('http://localhost/api/items?site_id=1');

    const second = at(sites, 1);
    expect(second.isPublic).toBe(false);
    expect(second.homepage).toBeNull();
    expect(second.navigation).toEqual([]);
  });

  it('site_rejects_a_missing_slug', () => {
    const error = parseErrorOf(() =>
      parseSite({
        '@id': 'http://localhost/api/sites/1',
        '@type': ['o:Site'],
        'o:id': 1,
        'o:theme': 'default',
        'o:title': 'My Site',
      }),
    );
    expect(error.path).toEqual(['o:slug']);
    expect(error.message).toBe('$.o:slug: expected a value');
  });
});

describe('parseApiResource', () => {
  it('api_resource_id_is_the_resource_name', () => {
    const resources = parseListOf(parseApiResource)(loadFixture('api-root.json'));
    expect(resources.map((resource) => resource.id)).toEqual([
      'items',
      'media',
      'users',
      'vocabularies',
    ]);
  });
});
