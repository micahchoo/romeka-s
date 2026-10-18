/**
 * The reads the page editor makes before it renders: the stored page, its site
 * and the option lists the block fields share. A spec's stubbed fetch answers
 * these the way the run's other reads answer, with the fixtures under
 * `tests/fixtures`.
 */
import { loadFixture } from './loadFixture';
import { jsonResponse, listResponse } from './stub';

export const editorReads = (url: URL): Response | null => {
  switch (url.pathname) {
    case '/api/site_pages':
      return listResponse([loadFixture('site-page-1.json')], 1);
    case '/api/sites/1':
      return jsonResponse(loadFixture('site-1.json'));
    case '/api/item_sets':
      return listResponse(loadFixture('item-sets-page.json'), 2);
    case '/api/resource_classes':
      return listResponse(loadFixture('resource-classes-page.json'), 2);
    default:
      return null;
  }
};
